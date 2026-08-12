//! Run a bash command under the profiler and print the tree its measured
//! calls made.

use clap::Parser;

use mb_resolver::bash::rig::{run, ExitStatus, Failure};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Recorded};

#[derive(Parser)]
#[command(name = "bashprof", about = "Time a tree of calls in a bash program")]
struct Cli {
    /// Print the tree as recorded — every call that began, ended or not —
    /// rather than as timings.
    #[arg(long)]
    as_recorded: bool,

    /// The wrapped command, program included — `bash build.bash`, or
    /// `make test`, whose own shells join too. Everything from the first
    /// plain word on is the subject's; a command that itself starts with a
    /// dash goes behind `--`.
    #[arg(trailing_var_arg = true, required = true)]
    argv: Vec<String>,
}

fn main() {
    let code = match Cli::try_parse() {
        Ok(cli) => profile(&cli.argv, cli.as_recorded)
            .map(ExitStatus::shell_code)
            .unwrap_or_else(|error| {
                eprintln!("bashprof: {error}");
                1
            }),
        Err(complaint) => {
            let _ = complaint.print();
            2
        }
    };

    std::process::exit(code);
}

/// The exit code is the subject's, so a profiled script is indistinguishable
/// from an unprofiled one.
///
/// A run that died inside a measured call still measured everything that
/// completed, and those measurements are printed: a tool reporting what it has
/// is the caller `Profile::of` splits its result for.
fn profile(argv: &[String], as_recorded: bool) -> Result<ExitStatus, Failure> {
    let ran = run(&BashProf, argv)?;
    let forest = recorded(&ran.session)?;

    if as_recorded {
        println!("{}", Recorded::render(&forest));
    } else {
        match Profile::of(&forest) {
            Ok(profile) => println!("{profile}"),
            Err(unfinished) => {
                println!("{}", unfinished.resolved);
                eprintln!("bashprof: {unfinished}");
            }
        }
    }

    if let Some(why) = ran.failed {
        eprintln!("bashprof: {why}");
    }

    Ok(ran.subject)
}
