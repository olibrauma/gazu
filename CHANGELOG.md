# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] — 2026-06-16

### Changed

- Updated sekien to 0.3.2.

## [0.2.0] — 2026-06-16

### Added

- CI now runs tests on macOS and Windows in addition to Linux.
- `util/check-html-formats.sh --check` verifies that `HTML_FORMATS` in
  `src/pandoc.rs` matches the installed pandoc version; the check runs in CI.

### Changed

- SVG files for non-HTML formats are now written to a `gazu/` subdirectory
  instead of the current directory. Clean up with `rm -rf gazu/`.
- Render failure warnings now print the Mermaid error on its own line, so
  multi-line errors (including source context and `^` pointer) display
  correctly.

### Fixed

- SVG output now includes proper `xmlns:xlink` declarations for namespaced
  attributes such as those produced by `click` directives. Previously, strict
  XML parsers (e.g. typst's usvg) would reject the file and fail the build.
  (Fixed in sekien 0.3.1.)

## [0.1.0] — 2026-06-16

### Added

- Pandoc JSON filter that converts ` ```mermaid ` code blocks to SVG.
- Batch rendering: all diagrams in a document are rendered in a single WebView
  session, paying the Xvfb / WebView startup cost only once.
- Generic AST traversal finds Mermaid blocks anywhere in the document
  (inside Div, BlockQuote, Table cells, etc.) without per-node-type
  special-casing.
- Format-aware output: inline SVG for HTML-passing formats; `Image` backed by
  an SVG file for formats that drop raw HTML (typst, etc.).
- Graceful degradation: a diagram that fails to parse is left as the original
  code block; a warning is printed to stderr and the rest of the document
  continues processing.
- `GAZU_CONFIG` environment variable accepts a Mermaid configuration JSON file
  (same format as `mmdc --configFile`).
- Prebuilt binaries for Linux x86_64, macOS arm64, and Windows x86_64.

[Unreleased]: https://github.com/olibrauma/gazu/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/olibrauma/gazu/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/olibrauma/gazu/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/olibrauma/gazu/releases/tag/v0.1.0
