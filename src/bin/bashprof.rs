//! Time a tree of calls in a bash program, and write what its measured calls
//! recorded.
//!
//! Two ways in, and they differ only in who started the shells. `run_bash_env`
//! starts a command line and reaches its whole process tree through
//! `BASH_ENV`; `serve` is started *by* a bash script and hands that script the
//! address to join. A span is the interval between two messages either way —
//! which is why both take the same options, from the same type.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use mb_resolver::bash::rig::{heard, Attended, Doing, Driving, Failure, Message, Said, Serving};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Recorded, Unfinished, Unread};

#[derive(Parser)]
#[command(name = "bashprof", about = "Time a tree of calls in a bash program")]
struct Cli {
    #[command(subcommand)]
    what: What,
}

#[derive(Subcommand)]
enum What {
    /// Profile a command line, reaching every shell it starts through
    /// BASH_ENV.
    #[command(name = "run_bash_env")]
    RunBashEnv {
        #[command(flatten)]
        reading: Reading,

        /// The wrapped command, program included — `bash build.bash`, or
        /// `make test`, whose own shells join too. Everything from the first
        /// plain word on is the subject's; a command that itself starts with a
        /// dash goes behind `--`.
        #[arg(trailing_var_arg = true, required = true)]
        argv: Vec<String>,
    },

    /// Profile for a bash script that started this process as a coprocess: it
    /// holds this process's standard input, and reads the address to join from
    /// its standard output.
    Serve {
        #[command(flatten)]
        reading: Reading,
    },
}

/// What to write and how far to read the run first — the same question in both
/// roles.
#[derive(Args)]
struct Reading {
    /// Where the reading goes. The subject owns both streams, so nothing of
    /// bashprof's is written to them but its own failures.
    #[arg(long)]
    into: PathBuf,

    /// How far the run is read before it is written, and in what.
    #[arg(long, value_enum, default_value_t = Output::Human)]
    output: Output,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Output {
    /// The measured tree, indented, one call per message.
    Human,

    /// The same tree as a JSON array of root spans.
    Tree,

    /// The recorded tree: the same array with every call that began, each
    /// tagged `"state": "ended"` or `"state": "unended"`.
    TreeWithErr,

    /// Every message the run heard, with the shell that sent it: one JSON
    /// object per line, before any of it is read as calls.
    Raw,
}

impl Reading {
    /// Truncate before a shell can join, so an unwritable path is known
    /// straight away and a run that reads as nothing leaves nothing earlier
    /// standing in for its reading.
    fn claim(&self) -> Result<(), Failure> {
        self.write(String::new())
    }

    fn write(&self, text: String) -> Result<(), Failure> {
        std::fs::write(&self.into, text).doing(|| format!("writing {}", self.into.display()))
    }

    /// What the shells said, read as far as this asks for, and written.
    fn keep(&self, shells: &[Attended<Vec<Message>>]) -> Result<(), Failure> {
        let text = self.written(&heard(shells))?;

        self.write(text + "\n")
    }

    /// What goes into the file, or what stopped it being knowable.
    ///
    /// `Human` and `Tree` are one reading in two hands, and the only one that
    /// can refuse: every entry of it claims a duration, and a call the shell
    /// died inside has none. The other two report what the run said, which is
    /// defined however it ended.
    fn written(&self, heard: &[Said<'_>]) -> Result<String, Failure> {
        match self.output {
            Output::Raw => heard
                .iter()
                .map(|said| {
                    let at = || format!("a message from pid {}", said.shell.pid);
                    serde_json::to_string(said).doing(at)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),

            Output::TreeWithErr => json(&salvaged(heard)),
            Output::Human => measured(heard).map(|profile| profile.to_string()),
            Output::Tree => measured(heard).and_then(|profile| json(&profile)),
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let code = match Cli::try_parse() {
        Ok(cli) => perform(&cli.what).await,
        // `--help` and `--version` are complaints too, and clap gives them
        // their own code — 0, where a real misuse is 2.
        Err(complaint) => {
            let _ = complaint.print();
            complaint.exit_code()
        }
    };

    std::process::exit(code);
}

async fn perform(what: &What) -> i32 {
    match what {
        What::RunBashEnv { reading, argv } => run(reading, argv).await,
        What::Serve { reading } => match serve(reading).await {
            Ok(()) => 0,
            Err(why) => {
                eprintln!("bashprof: {why}");
                1
            }
        },
    }
}

/// The subject's own status wherever the subject failed, so a profiled script
/// is indistinguishable from an unprofiled one. Where the subject succeeded
/// and bashprof could not write what was asked for, the failure is bashprof's
/// and so is the status.
async fn run(reading: &Reading, argv: &[String]) -> i32 {
    if let Err(why) = reading.claim() {
        eprintln!("bashprof: {why}");
        return 1;
    }

    match BashProf.run(argv).await {
        Err(why) => {
            eprintln!("bashprof: {why}");
            1
        }
        Ok(ran) => {
            let wrote = reading
                .keep(&ran.shells)
                .and_then(|()| ran.failed.map_or(Ok(()), Err))
                .map_err(|why| eprintln!("bashprof: {why}"));

            match (ran.subject.shell_code(), wrote) {
                (0, Err(())) => 1,
                (code, _) => code,
            }
        }
    }
}

/// Nothing here starts a shell or ends one, so there is no subject's status to
/// hand back — only whether the reading came out whole. The client's `BC_LEAVE`
/// waits for this process, so that status is what its own `set -e` sees.
async fn serve(reading: &Reading) -> Result<(), Failure> {
    reading.claim()?;

    let served = BashProf.serve_coprocess().await?;

    reading.keep(&served.shells)?;
    served.failed.map_or(Ok(()), Err)
}

/// The run as a forest, and a word on stderr for every source path a frame
/// names and does not have. Not a failure: a subject that changed directory
/// after sourcing, or a workspace the run threw away, leaves paths that were
/// true when they were written.
fn read(heard: &[Said<'_>]) -> Result<Vec<Recorded>, Unread> {
    let forest = recorded(heard);

    for path in Recorded::missing(forest.as_ref().unwrap_or_else(|unread| &unread.resolved)) {
        eprintln!("bashprof: no source at {}", path.display());
    }

    forest
}

/// What the run recorded, whether or not all of it read back. This output
/// reports what the run said, so a message the instrument wrote and mangled is
/// a word on stderr rather than the end of it.
fn salvaged(heard: &[Said<'_>]) -> Vec<Recorded> {
    read(heard).unwrap_or_else(|unread| {
        eprintln!("bashprof: {unread}");

        unread.resolved
    })
}

/// The run read as measurements, or why it has none to give. Every entry
/// claims a duration, so a partial reading is no more use here than none.
fn measured(heard: &[Said<'_>]) -> Result<Profile, Failure> {
    let forest = read(heard).map_err(|unread| Failure::new("reading the run", unread.to_string()))?;
    let reading = |unfinished: Unfinished<'_>| {
        Failure::new("reading the run as measurements", unfinished.to_string())
    };

    Profile::of(&forest).map_err(reading)
}

fn json<T: serde::Serialize>(what: &T) -> Result<String, Failure> {
    serde_json::to_string_pretty(what).doing(|| "serializing the tree".to_string())
}
