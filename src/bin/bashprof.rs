//! Run a bash command under the profiler and serialize what its measured
//! calls recorded.

use clap::{Parser, ValueEnum};

use mb_resolver::bash::rig::{run, Doing, Failure, Line};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Unfinished};

#[derive(Parser)]
#[command(name = "bashprof", about = "Time a tree of calls in a bash program")]
struct Cli {
    /// What to write to stdout.
    #[arg(long, value_enum, default_value_t = Output::Tree)]
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
    /// The measured tree: one JSON array of root spans. A call the shell died
    /// inside leaves no measurement, so this fails rather than report a tree
    /// that is quietly missing time.
    Tree,

    /// The recorded tree: the same array, with every call that began, whether
    /// or not it ended. An unended one carries `"ended": null`.
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

/// The subject's own status when it failed, so a profiled script is
/// indistinguishable from an unprofiled one. Where the subject succeeded and
/// bashprof could not produce what was asked for, the failure is bashprof's
/// and so is the status.
fn produce(cli: &Cli) -> i32 {
    match run(&BashProf, &cli.argv) {
        Err(why) => {
            eprintln!("bashprof: {why}");
            1
        }
        Ok(ran) => {
            let wrote = written(cli.output, &ran.session)
                .and_then(|text| {
                    println!("{text}");
                    ran.failed.map_or(Ok(()), Err)
                })
                .map_err(|why| eprintln!("bashprof: {why}"));

            match (ran.subject.shell_code(), wrote) {
                (0, Err(())) => 1,
                (code, _) => code,
            }
        }
    }
}

/// What goes to stdout, or what stopped it being knowable.
///
/// Only `Tree` refuses an unfinished run: it is the one output whose every
/// entry claims a duration, and a call the shell died inside has none. The
/// other two report what the run said, which is defined however it ended.
fn written(output: Output, heard: &[Line]) -> Result<String, Failure> {
    match output {
        Output::Raw => heard
            .iter()
            .map(|line| {
                let at = || format!("a message from pid {}", line.pid);
                serde_json::to_string(line).doing(at)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n")),

        Output::TreeWithErr => json(&recorded(heard)?),

        Output::Tree => {
            let forest = recorded(heard)?;
            let reading = |unfinished: Unfinished<'_>| {
                Failure::new("reading the run as measurements", unfinished.to_string())
            };

            json(&Profile::of(&forest).map_err(reading)?)
        }
    }
}

fn json<T: serde::Serialize>(what: &T) -> Result<String, Failure> {
    serde_json::to_string_pretty(what).doing(|| "serializing the tree".to_string())
}
