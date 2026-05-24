use std::io::{self, Write};

use anyhow::Context;
use serde::Serialize;

use crate::cli::OutputFormat;

pub fn print<T>(
    format: OutputFormat,
    value: &T,
    human: impl FnOnce() -> String,
) -> anyhow::Result<()>
where
    T: Serialize,
{
    match format {
        OutputFormat::Human => {
            println!("{}", human());
            Ok(())
        }
        OutputFormat::Json => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer_pretty(&mut handle, value).context("failed to write JSON")?;
            writeln!(handle).context("failed to finish JSON output")?;
            Ok(())
        }
    }
}
