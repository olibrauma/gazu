mod filter;

use anyhow::{Context, Result};
use std::io::{self, IsTerminal, Read};

fn usage() -> &'static str {
    "gazu — Pandoc filter for Mermaid diagrams

Usage:
  pandoc input.md -o output.html --filter gazu
  pandoc input.md -o output.pdf --pdf-engine=typst --filter gazu

Options:
  --version, -v          Show version
  --help, -h             Show this help

Environment:
  GAZU_CONFIG             Path to a mermaid.initialize() JSON config file
                          (same format as mmdc's --configFile)"
}

enum Command {
    Help,
    Version,
    Filter(String),
}

/// Determines what to do by combining CLI flags with runtime state (TTY check).
/// `Command::Filter` is only returned when stdin is a pipe — i.e. pandoc is
/// actually feeding an AST.
fn resolve_command() -> Command {
    let mut format = "html".to_owned();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => return Command::Help,
            "--version" | "-v" => return Command::Version,
            s if !s.starts_with('-') => format = s.to_owned(),
            _ => {}
        }
    }
    if io::stdin().is_terminal() {
        return Command::Help;
    }
    Command::Filter(format)
}

fn main() -> Result<()> {
    match resolve_command() {
        Command::Help => {
            println!("{}", usage());
            Ok(())
        }
        Command::Version => {
            println!(
                "gazu {} (mermaid.js {})",
                env!("CARGO_PKG_VERSION"),
                sekien::MERMAID_VERSION
            );
            Ok(())
        }
        Command::Filter(format) => {
            let config_json = std::env::var("GAZU_CONFIG")
                .ok()
                .map(|path| {
                    std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read GAZU_CONFIG '{path}'"))
                })
                .transpose()?;

            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .context("failed to read Pandoc AST from stdin")?;

            filter::filter(&input, &format, config_json.as_deref())
        }
    }
}
