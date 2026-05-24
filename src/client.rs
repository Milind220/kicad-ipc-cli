use std::time::Duration;

use anyhow::Context;
use kicad_ipc_rs::KiCadClientBlocking;

use crate::cli::Cli;

pub fn connect(cli: &Cli) -> anyhow::Result<KiCadClientBlocking> {
    let mut builder = KiCadClientBlocking::builder()
        .timeout(Duration::from_millis(cli.timeout_ms))
        .client_name("kicad-ipc-cli");

    if let Some(socket) = &cli.socket {
        builder = builder.socket_path(socket.clone());
    }
    if let Some(token) = &cli.token {
        builder = builder.token(token.clone());
    }

    builder.connect().context("failed to connect to KiCad IPC")
}
