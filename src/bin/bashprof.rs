//! Run a bash command under the profiler and write what its measured calls
//! recorded.

use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};

use mb_resolver::bash::rig::{run, Doing, Failure, Line};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Recorded, Unfinished};

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

    /// Every message the run heard, one JSON object per line, before any of it
    /// is read as calls.
    Raw,
}

fn main() {
    let code = match Cli::try_parse() {
        Ok(cli) => produce(&cli),
        Err(complaint) => {
            let _ = complaint.print();
            2
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

    match run(&BashProf, &cli.argv) {
        Err(why) => {
            eprintln!("bashprof: {why}");
            1
        }
        Ok(ran) => {
            let wrote = written(cli.output, &ran.session)
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
fn written(output: Output, heard: &[Line]) -> Result<String, Failure> {
    match output {
        Output::Raw => heard
            .iter()
            .map(|line| {
                let at = || format!("a message from pid {}", line.sent.pid);
                serde_json::to_string(line).doing(at)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n")),

        Output::TreeWithErr => json(&read(heard)?),
        Output::Human => measured(heard).map(|profile| profile.to_string()),
        Output::Tree => measured(heard).and_then(|profile| json(&profile)),
    }
}

/// The run as a forest, and a word on stderr for every source path a frame
/// names and does not have. Not a failure: a subject that changed directory
/// after sourcing, or a workspace the run threw away, leaves paths that were
/// true when they were written.
fn read(heard: &[Line]) -> Result<Vec<Recorded>, Failure> {
    let forest = recorded(heard)?;

    for path in Recorded::missing(&forest) {
        eprintln!("bashprof: no source at {}", path.display());
    }

    Ok(forest)
}

/// The run read as measurements, or why it has none to give.
fn measured(heard: &[Line]) -> Result<Profile, Failure> {
    let forest = read(heard)?;
    let reading = |unfinished: Unfinished<'_>| {
        Failure::new("reading the run as measurements", unfinished.to_string())
    };

    Profile::of(&forest).map_err(reading)
}

fn json<T: serde::Serialize>(what: &T) -> Result<String, Failure> {
    serde_json::to_string_pretty(what).doing(|| "serializing the tree".to_string())
}
