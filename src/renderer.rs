//! Thin wrapper around the sekien lib (`render_stream`).
//!
//! gazu processes everything in one run and then exits, so it calls
//! `sekien::render_stream` directly instead of spawning a child process.
//! The WebView/Xvfb initialization cost is paid only once per process.

use anyhow::Result;
use sekien::{render_stream, RenderOutcome};

/// Result of rendering a single block.
pub enum BlockOutcome {
    Rendered(String),
    /// Mermaid failed to parse/render. Holds the error message.
    Failed(String),
}

/// Renders `diagrams` in a single batch via sekien, returning `BlockOutcome`
/// in input order.
pub fn render_blocks(
    diagrams: Vec<String>,
    config_json: Option<&str>,
) -> Result<Vec<BlockOutcome>> {
    if diagrams.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::with_capacity(diagrams.len());
    render_stream(diagrams, config_json, |_id, outcome| {
        results.push(match outcome {
            RenderOutcome::Svg(svg) => BlockOutcome::Rendered(svg),
            RenderOutcome::Error(err) => BlockOutcome::Failed(err),
        });
    })?;

    Ok(results)
}
