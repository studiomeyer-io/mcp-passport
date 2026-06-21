//! `mcp-passport` — a thin CLI over the [`mcp_passport`] library.
//!
//! ```text
//! mcp-passport                       # validate ./server.json
//! mcp-passport path/to/server.json   # validate a specific file
//! mcp-passport ./my-server           # find + validate server.json in a dir
//! mcp-passport --format sarif         # GitHub code scanning
//! mcp-passport --fail-on warning      # gate CI on warnings too
//! ```

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use mcp_passport::report::render;
use mcp_passport::{resolve_server_json, sarif, validate, Level, Report};

#[derive(Parser, Debug)]
#[command(
    name = "mcp-passport",
    version,
    about = "Publish-readiness validator for the MCP Registry (server.json + manifest consistency)."
)]
struct Cli {
    /// A server.json file, or a directory containing one.
    #[arg(default_value = ".", value_name = "PATH")]
    path: PathBuf,
    /// Exit non-zero when a finding at or above this level is present.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Format {
    Human,
    Sarif,
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum FailOn {
    Error,
    Warning,
    Info,
    Never,
}

fn main() {
    match run(Cli::parse()) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(2);
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    let file = resolve_server_json(&cli.path)?;
    let report = validate(&file)?;
    let target = file.to_string_lossy().to_string();

    match cli.format {
        Format::Human => print!("{}", render(&report, &target)),
        Format::Sarif => println!(
            "{}",
            serde_json::to_string_pretty(&sarif::to_sarif(&report))?
        ),
        Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    Ok(exit_code(&report, cli.fail_on))
}

fn exit_code(report: &Report, fail_on: FailOn) -> i32 {
    let threshold = match fail_on {
        FailOn::Error => Level::Error,
        FailOn::Warning => Level::Warning,
        FailOn::Info => Level::Info,
        FailOn::Never => return 0,
    };
    if report.has_at_least(threshold) {
        1
    } else {
        0
    }
}
