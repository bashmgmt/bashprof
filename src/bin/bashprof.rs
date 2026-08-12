//! Run a bash command under the profiler and print the tree its measured
//! calls made.

use clap::{Parser, Subcommand};

use mb_resolver::bash::rig::{run, ExitStatus, Failure};
use mb_resolver::bashprof::{recorded, BashProf, Profile, Recorded, POLYFILL};

#[derive(Parser)]
#[command(name = "bashprof", about = "Time a tree of calls in a bash program")]
struct Cli {
    #[command(subcommand)]
    what: What,
}

#[derive(Subcommand)]
enum What {
    /// Run a bash command and print what it measured.
    Run {
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
    },

    /// Print the client-side no-op stub.
    Polyfill,
}

fn main() {
    let code = match Cli::try_parse() {
        Ok(cli) => perform(cli.what).unwrap_or_else(|error| {
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

fn perform(what: What) -> Result<i32, Failure> {
    match what {
        What::Polyfill => {
            print!("{POLYFILL}");
            Ok(0)
        }
        What::Run { as_recorded, argv } => {
            profile(&argv, as_recorded).map(ExitStatus::shell_code)
        }
    }
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
