mod cli;
mod client;
mod commands;
mod groups;
mod model;
mod output;
mod units;

use std::process::ExitCode;

use anyhow::Error;
use clap::Parser;
use kicad_ipc_rs::KiCadError;

use crate::cli::Cli;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            print_kicad_hint(&err);
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    commands::dispatch(cli)
}

fn print_kicad_hint(err: &Error) {
    for cause in err.chain() {
        if let Some(kicad) = cause.downcast_ref::<KiCadError>() {
            match kicad {
                KiCadError::BoardNotOpen | KiCadError::SocketUnavailable { .. } => {
                    eprintln!(
                        "hint: launch KiCad 10.0.1+, enable Preferences > Plugins > Enable IPC API, open a project, and open the PCB editor."
                    );
                    return;
                }
                KiCadError::ApiStatus { code, message } if code == "AS_UNHANDLED" => {
                    eprintln!(
                        "hint: KiCad reported this API command as unavailable (`{message}`). Check the KiCad version and IPC settings."
                    );
                    return;
                }
                _ => {}
            }
        }
    }
}
