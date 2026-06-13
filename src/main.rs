mod pandoc;
mod renderer;

use anyhow::{Context, Result};
use std::io::{self, Read};

const LUA_FILTER: &str = include_str!("../assets/sekien.lua");

fn usage() -> &'static str {
    "sekien-pandoc — Pandoc filter for Mermaid diagrams

Usage:
  pandoc input.md -o output.html --filter sekien-pandoc
  pandoc input.md -o output.pdf --pdf-engine=typst \\
    --filter sekien-pandoc \\
    --lua-filter <(sekien-pandoc --print-lua-filter)

Options:
  --print-lua-filter     Print the bundled Lua filter for non-HTML PDF output
  --version, -v          Show version
  --help, -h             Show this help"
}

fn main() -> Result<()> {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(());
            }
            "--version" | "-v" => {
                println!(
                    "sekien-pandoc {} (mermaid.js {})",
                    env!("CARGO_PKG_VERSION"),
                    sekien::MERMAID_VERSION
                );
                return Ok(());
            }
            "--print-lua-filter" => {
                print!("{LUA_FILTER}");
                return Ok(());
            }
            _ => {} // Pandoc が渡す output format 等の未知引数は無視する
        }
    }

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read Pandoc AST from stdin")?;

    pandoc::filter(&input)
}
