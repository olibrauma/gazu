# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] — 2026-10-01

### Changed

- **Breaking**: Updated sekien to 0.5.1, which bundles mermaid.js 12.0.0
  (matching mermaid-cli 12.0.0). The same document now renders differently
  by default:
  - ELK layout instead of dagre (flowchart, state, class, ER, requirement
    and use case diagrams).
  - The `redux-color` theme and `neo` look for flowchart, sequence, class,
    state, ER, requirement and use case diagrams.
  - Flowchart and state nodes have a minimum width of 120px, and flowchart
    labels wrap at 120px instead of 200px.

  See [Output from gazu 0.3 and earlier](README.md#output-from-gazu-03-and-earlier)
  for a `GAZU_CONFIG` that restores the previous output.
- **Breaking**: The `flowchart.defaultRenderer`, `class.defaultRenderer` and
  `state.defaultRenderer` config options are now ignored by mermaid.js. Use
  the top-level `layout` option instead.
- **Breaking**: The OS WebView must now be WebKitGTK 2.44+ on Linux or
  Safari 17.4+ on macOS (mermaid.js 12 targets ES2024).
- With the new defaults, rendering is somewhat slower and uses more memory.
  On Linux x86_64, `util/bench/fixture.md` (3 diagrams) takes about 20%
  longer than with 0.3.3 (median 1.07 s → 1.30 s, running the versions
  alternately), and peak RSS is about 5% higher (541 MB → 566 MB). With the
  `GAZU_CONFIG` above it is about 10% faster than 0.3.3 (0.96 s); the ELK
  layout code is then never loaded.
- The README's mermaid-filter comparison is re-measured for Linux with
  0.4.0.

### Fixed

- Windows: rendering failed with WebView2 error 0x80070057 in every earlier
  version (sekien#5). Fixed in sekien 0.5.1; gazu's integration tests now run
  on the GitHub Actions Windows runner in CI. Not yet tried on a desktop
  Windows machine.

## [0.3.3] — 2026-09-29

### Changed

- Updated sekien to 0.4.3 (mermaid.js unchanged at 11.17.2). It updates wry
  to 0.57 and tao to 0.37, and no longer needs the libdbus development
  headers to build.
- If sekien's rendering window closes before every diagram is rendered, gazu
  now exits with an error instead of leaving the remaining Mermaid blocks
  unrendered without a warning (fixed in sekien 0.4.3).
- Declared `rust-version = "1.88"`, the minimum required by sekien 0.4.3 and
  gazu's other dependencies. Earlier versions declared none.

## [0.3.2] — 2026-09-05

### Changed

- Updated sekien to 0.4.2 (mermaid.js 11.17.2, matching mermaid-cli 11.17.0).

## [0.3.1] — 2026-07-09

### Changed

- Updated sekien to 0.4.1 (mermaid.js 11.16.0).

## [0.3.0] — 2026-06-17

### Added

- `gazu` now shows help when invoked directly from a terminal (stdin is a TTY),
  rather than blocking on input. Pandoc always pipes the AST to stdin, so TTY
  stdin reliably indicates a direct invocation with no useful input to process.

### Changed

- Updated sekien to 0.4.0 (mermaid.js 11.15.0).
- `GAZU_CONFIG` error messages now follow sekien's style. File-not-found:
  `failed to read GAZU_CONFIG '<path>': <os error>`. Invalid JSON or
  non-object: reported by sekien as `invalid config_json: <detail>`.

## [0.2.0] — 2026-06-17

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
- Updated sekien to 0.3.2.

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

[Unreleased]: https://github.com/olibrauma/gazu/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/olibrauma/gazu/compare/v0.3.3...v0.4.0
[0.3.3]: https://github.com/olibrauma/gazu/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/olibrauma/gazu/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/olibrauma/gazu/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/olibrauma/gazu/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/olibrauma/gazu/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/olibrauma/gazu/releases/tag/v0.1.0
