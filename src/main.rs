mod adapter;
mod cli;
mod design;
mod error;
mod git;
mod home;
mod id;
mod output;
mod paths;
mod repository;
mod serve;
mod setup;
mod store;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use cli::{Cli, Command, DevCommand, Parsed, Runtime};
use error::SkaldError;
use output::{Format, Output};
use store::Store;

fn main() -> ExitCode {
    run(
        std::env::args_os(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

fn run(
    args: impl IntoIterator<Item = OsString>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let args: Vec<OsString> = args.into_iter().collect();
    let format = Format::from_args(&args);
    let result = cli::parse(&args).and_then(|parsed| match parsed {
        Parsed::Print(text) => Ok((text, 0)),
        Parsed::Run(cli) => dispatch(cli, stdout).map(|output| {
            let exit_code = output.exit_code();
            (output.encode(format), exit_code)
        }),
    });
    let (text, exit_code) = result.unwrap_or_else(|err| {
        let exit_code = err.exit_code;
        (err.into_output().encode(format), exit_code)
    });
    if let Err(err) = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        let _ = writeln!(stderr, "skald: cannot write to stdout: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::from(exit_code)
}

fn dispatch(cli: Cli, stdout: &mut impl Write) -> Result<Output, SkaldError> {
    match cli.command {
        None => home::view(&current_dir()?),
        Some(Command::Setup { no_plugin }) => setup::run(no_plugin),
        Some(Command::Hook {
            runtime: Runtime::ClaudeCode,
        }) => Ok(current_dir()
            .and_then(|cwd| home::view(&cwd))
            .unwrap_or_else(home::unavailable)),
        Some(Command::Design { command }) => {
            // Every design command needs the store, so a store this program cannot read is
            // reported before anything else.
            let mut store = Store::open(&paths::data_dir()?)?;
            design::run(command, &mut store, &current_dir()?)
        }
        Some(Command::Dev {
            command: DevCommand::Seed { designs },
        }) => {
            let mut store = Store::open(&paths::data_dir()?)?;
            design::seed(&mut store, &current_dir()?, designs)
        }
        Some(Command::Serve {
            design,
            version,
            page_from_stdin,
            idle_timeout,
            port,
        }) => {
            let page = match (design, version) {
                (Some(design), Some(version)) if !page_from_stdin => {
                    let mut store = Store::open(&paths::data_dir()?)?;
                    let (_, page) =
                        design::served(&mut store, &current_dir()?, &design, Some(version))?;
                    page
                }
                _ => {
                    let mut page = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut page).map_err(
                        |err| {
                            SkaldError::new("serve-failed", format!("cannot read the page: {err}"))
                        },
                    )?;
                    page
                }
            };
            serve::serve(page, port, idle_timeout, stdout)?;
            Ok(Output::Raw(String::new()))
        }
    }
}

fn current_dir() -> Result<PathBuf, SkaldError> {
    std::env::current_dir().map_err(|err| {
        SkaldError::new(
            "no-working-directory",
            format!("cannot read the working directory: {err}"),
        )
    })
}
