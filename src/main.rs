mod pandoc;
mod renderer;

use anyhow::{bail, Context, Result};
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

/// Reads the JSON file pointed to by `GAZU_CONFIG` and turns it into a
/// config_json string suitable for `render_stream`.
///
/// Expects the same format as mmdc's `--configFile` (a JSON object passed to
/// mermaid.initialize()).
fn load_config_json(path: &str) -> Result<String> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read config file '{path}' (from GAZU_CONFIG)"))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("invalid JSON in '{path}'"))?;
    if !value.is_object() {
        bail!("'{path}': expected a JSON object");
    }
    Ok(value.to_string())
}

fn main() -> Result<()> {
    // Pandoc always pipes the AST to stdin, so a TTY stdin means the user
    // invoked gazu directly. Show help instead of blocking on input.
    if io::stdin().is_terminal() {
        println!("{}", usage());
        return Ok(());
    }

    // Pandoc passes the output format name as the sole positional argument
    // when invoking a filter (e.g. "html", "typst", "latex").
    let mut format = "html".to_owned();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            "--version" | "-v" => {
                println!(
                    "gazu {} (mermaid.js {})",
                    env!("CARGO_PKG_VERSION"),
                    sekien::MERMAID_VERSION
                );
                return Ok(());
            }
            _ if !arg.starts_with('-') => format = arg,
            _ => {} // ignore unknown flags
        }
    }

    let config_json = match std::env::var("GAZU_CONFIG") {
        Ok(path) => Some(load_config_json(&path)?),
        Err(_) => None,
    };

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read Pandoc AST from stdin")?;

    pandoc::filter(&input, &format, config_json.as_deref())
}
