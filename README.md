# gazu — Pandoc filter for Mermaid

A Pandoc filter for Mermaid. **Light**, **small**, and **fast**.

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

On Windows, gazu 0.3 and earlier could not render at all (WebView2 error
0x80070057, [sekien#5](https://github.com/olibrauma/sekien/issues/5)). gazu
0.4 renders on the GitHub Actions Windows runner, where its tests run in CI,
but has not yet been tried on a desktop Windows machine.

### WebView version

The bundled mermaid.js 12 targets ES2024, so the OS WebView must be recent
enough: WebKitGTK 2.44 or later on Linux (on Ubuntu 22.04, the
`jammy-updates` package rather than the original 2.36), and Safari 17.4 or
later on macOS (WKWebView uses the system WebKit, which is updated with
Safari). WebView2 on Windows is evergreen.

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

### Output from gazu 0.3 and earlier

gazu 0.4 bundles mermaid.js 12, which changed the defaults: ELK layout
instead of dagre, the `redux-color` theme and `neo` look, and narrower
flowchart/state nodes and label wrapping. To render as gazu 0.3 (mermaid.js
11) did, use this `GAZU_CONFIG`:

```json
{
  "theme": "default",
  "look": "classic",
  "flowchart": { "layout": "dagre", "minNodeWidth": 0, "wrappingWidth": 200 },
  "state": { "layout": "dagre", "minNodeWidth": 0, "wrappingWidth": 200 },
  "class": { "layout": "dagre" },
  "er": { "layout": "dagre" },
  "requirement": { "layout": "dagre" }
}
```

Set `layout` per diagram type as above, not at the top level: a top-level
`layout: "dagre"` also moves mindmaps off their own layout. Mindmaps shift
by a few pixels either way.

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

**Linux x86_64** (gazu 0.4.0)

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Install size | **7.1 MB** | ~568 MB | **99% smaller** |
| Speed (3 diagrams) | **~1.3 s** | ~7.9 s | **~6x faster** |
| Memory (RSS) | **~566 MB** | ~855 MB | **~34% less** |

**Apple Silicon (M-series)** (gazu 0.1.0, mermaid.js 11.14.0; not yet
re-measured for 0.4)

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Speed (3 diagrams) | **403 ms** | 4.60 s | **~11x faster** |
| Memory (RSS) | **87 MB** | 634 MB | **~86% less** |

mermaid-filter spawns `mmdc` (Puppeteer/Chromium) per block; gazu renders the
whole document in one batch.

- `util/bench/fixture.md` (3 diagrams) — see `./util/bench/bench.sh`
- Linux: gazu 0.4.0 (mermaid.js 12.0.0) vs. mermaid-filter 1.4.7 (mmdc
  10.9.1, mermaid.js 10.9.6). Speed is the median of 10 runs alternating
  between the two filters; RSS is the median of 10 runs of `bench.sh`.
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
