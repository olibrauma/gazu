# gazu — Pandoc filter for Mermaid

A Pandoc filter for Mermaid. **Fast**, **light**, **small**, and **OS-native**.

## Install

```bash
cargo install gazu
```

On Linux, WebKitGTK development packages are required (Ubuntu example):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

## Runtime requirements

### Linux

gazu launches its own Xvfb and ignores any existing display. Install Xvfb if
it isn't already present:

```bash
apt install xvfb                  # Debian / Ubuntu
dnf install xorg-x11-server-Xvfb  # Fedora
```

### macOS / Windows

A display connection is required (uses the OS-native WebView: WKWebView /
WebView2).

## Usage

```bash
# HTML
pandoc input.md -o output.html --filter gazu

# PDF via weasyprint
pandoc input.md -o output.pdf --pdf-engine=weasyprint --filter gazu

# PDF via typst
pandoc input.md -o output.pdf --pdf-engine=typst --filter gazu -V mainfont="Noto Sans"
```

Depending on the output format, gazu may write SVG files to a `gazu/`
subdirectory of the current directory. See
[Behavior by output format](#behavior-by-output-format). For PDF output, see
[Notes → PDF output](#pdf-output).

## CLI options

| Option | Description |
|---|---|
| `--version`, `-v` | Show version |
| `--help`, `-h` | Show help |

## Mermaid configuration

Set `GAZU_CONFIG` to a JSON file. Same
format as [mmdc](https://github.com/mermaid-js/mermaid-cli)'s `--configFile`:

```json
{
  "theme": "dark",
  "flowchart": { "curve": "basis" }
}
```

```bash
GAZU_CONFIG=mermaid-config.json \
  pandoc input.md -o output.html --filter gazu
```

## Behavior by output format

gazu embeds diagrams two ways, depending on the output format (`-t`/`-o`):

### Inline SVG

Formats that pass through raw HTML embed `<svg>...</svg>` directly, no file
written:

- HTML / slides: `html`, `html4`, `html5`, `s5`, `slidy`, `slideous`,
  `dzslides`, `revealjs`
- Markdown variants: `markdown`, `markdown_github`, `markdown_mmd`,
  `markdown_phpextra`, `markdown_strict`, `commonmark`, `commonmark_x`, `gfm`
- Others: `org`, `rst`, `mediawiki`, `muse`, `textile`, `docbook4`

### SVG file + Image

Other formats (`typst`, `latex`, etc.) drop raw HTML. gazu writes
`gazu/<hash>.svg` to a `gazu/` subdirectory (created if absent) and embeds
it as an `Image`. The files remain after conversion and can be removed with
`rm -rf gazu/`.

## Notes

### On failure

A diagram that fails to parse or render is left as the original
` ```mermaid ` code block, with a warning on stderr.

### PDF output

LaTeX-based `--pdf-engine`s (`pdflatex`, `xelatex`, `lualatex`, ...) need the
`svg` LaTeX package, `--shell-escape`, and `rsvg-convert` or `inkscape` on
PATH to render the embedded SVG — without them, the PDF build fails. Use
`--pdf-engine=weasyprint` or `--pdf-engine=typst` instead (see
[Usage](#usage)).

## vs mermaid-filter

gazu is smaller, faster, and lighter than
[mermaid-filter](https://github.com/raghur/mermaid-filter):

**Linux x86_64**

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Install size | **5.0 MB** | ~568 MB | **99% smaller** |
| Speed (3 diagrams) | **~2.0 s** | ~14.8 s | **~7x faster** |
| Memory (RSS) | **~446 MB** | ~849 MB | **~47% less** |

**Apple Silicon (M-series)**

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Speed (3 diagrams) | **403 ms** | 4.60 s | **~11x faster** |
| Memory (RSS) | **87 MB** | 634 MB | **~86% less** |

mermaid-filter spawns `mmdc` (Puppeteer/Chromium) per block; gazu renders the
whole document in one batch.

- Median of 10 runs, `util/bench/fixture.md` (3 diagrams) — see `./util/bench/bench.sh`
- Both use mermaid.js 11.14.0 (mermaid-filter 1.4.x / mmdc 11.14.0)
- Speed/Memory: filter process + children (Xvfb, WebKit, mmdc, Chromium), not pandoc itself
- Install size: gazu's binary vs. mermaid-filter's npm package + Puppeteer's Chromium download (Linux only)
- On Apple Silicon, mermaid-filter's bundled Chromium runs under Rosetta 2 (no
  native arm64 build for the pinned Puppeteer/Chromium revision) — part of
  that gap reflects translation overhead, not just gazu vs.
  mermaid-filter's architecture.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Bundled Assets

gazu embeds `mermaid.js` (via [sekien](https://github.com/olibrauma/sekien)).

- `mermaid.js`: Licensed under the [MIT License](mermaid.LICENSE). Copyright (c) 2014 - 2024 Knut Sveidqvist and contributors.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
