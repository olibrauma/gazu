//! Thin wrapper around the sekien lib (`render_stream`).
//!
//! gazu processes everything in one run and then exits, so it calls
//! `sekien::render_stream` directly instead of spawning a child process.
//! The WebView/Xvfb initialization cost is paid only once per process.

use anyhow::Result;
use sekien::{render_stream, RenderOutcome};
use std::sync::mpsc;

/// Result of rendering a single block.
pub enum BlockOutcome {
    Rendered(String),
    /// Mermaid failed to parse/render. Holds the error message.
    Failed(String),
}

/// Renders `diagrams` in a single batch via sekien, returning `BlockOutcome`
/// in input order.
///
/// `render_stream` calls `on_result` once per diagram in the same order as
/// `diagrams`, so we can just forward each result to a channel and collect.
pub fn render_blocks(
    diagrams: Vec<String>,
    config_json: Option<&str>,
) -> Result<Vec<BlockOutcome>> {
    if diagrams.is_empty() {
        return Ok(Vec::new());
    }

    let (tx, rx) = mpsc::channel();
    render_stream(diagrams, config_json, move |_id, outcome| {
        let mapped = match outcome {
            RenderOutcome::Svg(svg) => BlockOutcome::Rendered(svg),
            RenderOutcome::Error(err) => BlockOutcome::Failed(err),
        };
        let _ = tx.send(mapped);
    })?;

    Ok(rx.into_iter().collect())
}
