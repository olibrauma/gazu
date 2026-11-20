//! gazu — Pandoc filter and library for Mermaid diagrams.
//!
//! Renders Mermaid code blocks to SVG without Node.js or Chromium,
//! using the OS-native WebView via `sekien`.
//!
//! # Using gazu as a library
//!
//! Caller projects can manipulate Pandoc AST objects directly in memory,
//! pass Mermaid configuration directly as `serde_json::Value` or string objects
//! (no configuration file needed), and choose whether to write SVG files to disk
//! or keep everything in memory.
//!
//! ## In-memory AST filtering (no files created)
//!
//! ```no_run
//! use gazu::{filter_ast_value_in_memory, FilterOptions};
//! use serde_json::json;
//!
//! let mut ast = json!({
//!     "pandoc-api-version": [1, 23],
//!     "meta": {},
//!     "blocks": [
//!         {
//!             "t": "CodeBlock",
//!             "c": [["", ["mermaid"], []], "graph LR\n  A --> B"]
//!         }
//!     ]
//! });
//!
//! // Pass config object directly as serde_json::Value:
//! let config = json!({ "theme": "dark" });
//!
//! // Process AST in-memory without creating any files on disk:
//! let result = filter_ast_value_in_memory(&mut ast, "html", Some(&config)).unwrap();
//! assert!(result.warnings.is_empty());
//! ```

pub mod filter;

pub use filter::*;
pub use sekien::RenderOutcome;

/// gazu crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bundled mermaid.js version.
pub const MERMAID_VERSION: &str = sekien::MERMAID_VERSION;

/// Returns the gazu library version.
#[must_use]
pub fn version() -> &'static str {
    VERSION
}

/// Returns the bundled mermaid.js version.
#[must_use]
pub fn mermaid_version() -> &'static str {
    MERMAID_VERSION
}

/// Returns the gazu CLI usage instructions string.
#[must_use]
pub fn usage() -> &'static str {
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
