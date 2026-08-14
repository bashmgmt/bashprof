//! Run a bash command under the profiler and write what its measured calls
//! recorded.

use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};

use mb_resolver::bash::rig::{heard, Doing, Failure, Master, Said};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Recorded, Unfinished, Unread};

#[derive(Parser)]
#[command(name = "bashprof", about = "Time a tree of calls in a bash program")]
struct Cli {
    /// Where the reading goes. The subject owns both streams, so nothing of
    /// bashprof's is written to them but its own failures.
    #[arg(long)]
    into: PathBuf,

    /// How far the run is read before it is written, and in what.
    #[arg(long, value_enum, default_value_t = Output::Human)]
    output: Output,

    /// The wrapped command, program included — `bash build.bash`, or
    /// `make test`, whose own shells join too. Everything from the first
    /// plain word on is the subject's; a command that itself starts with a
    /// dash goes behind `--`.
    #[arg(trailing_var_arg = true, required = true)]
    argv: Vec<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Output {
    /// The measured tree, indented, one call per line.
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

fn main() {
    let code = match Cli::try_parse() {
        Ok(cli) => produce(&cli),
        // `--help` and `--version` are complaints too, and clap gives them
        // their own code — 0, where a real misuse is 2.
        Err(complaint) => {
            let _ = complaint.print();
            complaint.exit_code()
        }
    };

    std::process::exit(code);
}

/// The subject's own status wherever the subject failed, so a profiled script
/// is indistinguishable from an unprofiled one. Where the subject succeeded
/// and bashprof could not write what was asked for, the failure is bashprof's
/// and so is the status.
fn produce(cli: &Cli) -> i32 {
    // Truncated before the subject starts, so an unwritable path is known
    // straight away and a run that reads as nothing leaves nothing earlier
    // standing in for its reading.
    if let Err(why) = write(&cli.into, String::new()) {
        eprintln!("bashprof: {why}");
        return 1;
    }

    match BashProf.run(&cli.argv) {
        Err(why) => {
            eprintln!("bashprof: {why}");
            1
        }
        Ok(ran) => {
            let wrote = written(cli.output, &heard(&ran.shells))
                .and_then(|text| write(&cli.into, text + "\n"))
                .and_then(|()| ran.failed.map_or(Ok(()), Err))
                .map_err(|why| eprintln!("bashprof: {why}"));

            match (ran.subject.shell_code(), wrote) {
                (0, Err(())) => 1,
                (code, _) => code,
            }
        }
    }
}

fn write(into: &Path, text: String) -> Result<(), Failure> {
    std::fs::write(into, text).doing(|| format!("writing {}", into.display()))
}

/// What goes into the file, or what stopped it being knowable.
///
/// `Human` and `Tree` are one reading in two hands, and the only one that can
/// refuse: every entry of it claims a duration, and a call the shell died
/// inside has none. The other two report what the run said, which is defined
/// however it ended.
fn written(output: Output, heard: &[Said<'_>]) -> Result<String, Failure> {
    match output {
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
