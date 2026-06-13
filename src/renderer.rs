//! 薄い sekien lib (`render_stream`) wrapper。
//!
//! sekien-pandoc は 1 実行でまとめて処理して終了するため、子プロセスではなく
//! `sekien::render_stream` を直接呼び出す。WebView/Xvfb の初期化コストは
//! プロセス全体で 1 回のみ。

use anyhow::Result;
use sekien::{render_stream, RenderConfig, RenderOutcome};
use std::sync::mpsc;

/// 1 ブロック分の render 結果。
pub enum BlockOutcome {
    Rendered(String),
    /// Mermaid の解析・描画に失敗した。エラーメッセージを保持する。
    Failed(String),
}

/// `diagrams` を sekien に一括 render させ、入力順の `BlockOutcome` を返す。
///
/// `render_stream` は `on_result` を `diagrams` と同じ順序で 1 件ずつ呼び出す
/// ため、結果をそのままチャネルに送って `collect` するだけでよい。
pub fn render_blocks(diagrams: Vec<String>) -> Result<Vec<BlockOutcome>> {
    if diagrams.is_empty() {
        return Ok(Vec::new());
    }

    let (tx, rx) = mpsc::channel();
    let config = RenderConfig::default();
    render_stream(diagrams, &config, move |_id, outcome| {
        let mapped = match outcome {
            RenderOutcome::Svg(svg) => BlockOutcome::Rendered(svg),
            RenderOutcome::Error(err) => BlockOutcome::Failed(err),
        };
        let _ = tx.send(mapped);
    })?;

    Ok(rx.into_iter().collect())
}
