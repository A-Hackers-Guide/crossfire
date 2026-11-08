//! `crossfire-engine` — CLI sidecar for the Electron shell.
//!
//! The final result is printed as one JSON document on stdout; progress events
//! are printed as one JSON object per line on stderr. On failure stdout gets
//! `{"error": "..."}` and the exit code is 1.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossfire_engine::{
    self as engine, BuildRequest, ConvertRequest, Os, ProgressEvent, ScanReport,
};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "crossfire-engine",
    version,
    about = "Crossfire conversion engine"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan a project directory and print a ScanReport.
    Scan { path: PathBuf },
    /// Print questions for a scan report (JSON file, or - for stdin).
    Questions {
        scan: PathBuf,
        /// Comma-separated targets: windows,macos,linux,freebsd
        #[arg(long, default_value = "windows,macos,linux,freebsd")]
        targets: String,
    },
    /// Convert using a ConvertRequest JSON file (or - for stdin).
    Convert { request: PathBuf },
    /// Build using a BuildRequest JSON file (or - for stdin).
    Build { request: PathBuf },
    /// List detected toolchains.
    Toolchains,
    /// Run a Windows .exe through Game Porting Toolkit / Wine.
    Gptk { exe: PathBuf },
    /// Print the host OS id.
    Host,
    /// Print the number of embedded rules.
    Rules,
}

fn read_input(p: &PathBuf) -> Result<String> {
    if p.as_os_str() == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        Ok(s)
    } else {
        std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))
    }
}

fn emit(e: ProgressEvent) {
    if let Ok(line) = serde_json::to_string(&e) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{line}");
    }
}

fn print<T: Serialize>(v: &T) -> Result<()> {
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, v)?;
    writeln!(out)?;
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Scan { path } => print(&engine::scan(&path, &emit)?),
        Cmd::Questions { scan, targets } => {
            let report: ScanReport =
                serde_json::from_str(&read_input(&scan)?).context("parsing scan report")?;
            let targets: Vec<Os> = targets.split(',').filter_map(Os::parse).collect();
            print(&engine::questions(&report, &targets))
        }
        Cmd::Convert { request } => {
            let req: ConvertRequest =
                serde_json::from_str(&read_input(&request)?).context("parsing convert request")?;
            print(&engine::convert(&req, &emit)?)
        }
        Cmd::Build { request } => {
            let req: BuildRequest =
                serde_json::from_str(&read_input(&request)?).context("parsing build request")?;
            print(&engine::build(&req, &emit)?)
        }
        Cmd::Toolchains => print(&engine::toolchains()),
        Cmd::Gptk { exe } => print(&engine::test_with_gptk(&exe, &emit)?),
        Cmd::Host => print(&engine::host_os()),
        Cmd::Rules => print(&engine::rule_count()),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        let msg = format!("{e:#}");
        emit(ProgressEvent::new(
            engine::Stage::Scan,
            engine::Level::Error,
            -1.0,
            msg.clone(),
        ));
        let _ = print(&serde_json::json!({ "error": msg }));
        std::process::exit(1);
    }
}
