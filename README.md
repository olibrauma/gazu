# gazu — Pandoc filter for Mermaid

Pandoc 文書中の Mermaid コードブロックを SVG に変換するフィルタ。
文書内の全ブロックを 1 回の起動でまとめてレンダリングする。

## インストール

```bash
cargo install gazu
```

Linux でのビルドには WebKitGTK の開発パッケージが必要 (Ubuntu の例):

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
```

## 実行時の前提

gazu は内部で OS ネイティブ WebView を起動して Mermaid をレンダリングする。

Linux では起動時に内部で Xvfb を立ち上げるため、xvfb が必要:

```bash
apt install xvfb       # Debian / Ubuntu
dnf install xorg-x11-server-Xvfb  # Fedora
```

macOS / Windows ではディスプレイ接続が必要。

## 使い方

### HTML 出力

```bash
pandoc input.md -o output.html --filter gazu
```

文書中の Mermaid コードブロック（` ```mermaid ` で始まるもの）を
`<svg>` に置換して AST に埋め込む。Mermaid の解析に失敗したブロックは
元の `CodeBlock` をそのまま残し（graceful fallback）、stderr に warning を出す。

### PDF 出力

#### weasyprint (HTML 経由)

```bash
pandoc input.md -o output.pdf \
  --pdf-engine=weasyprint \
  --filter gazu
```

#### typst

```bash
pandoc input.md -o output.pdf \
  --pdf-engine=typst \
  --filter gazu \
  -V mainfont="Noto Sans"
```

typst は raw HTML を drop するため、gazu は SVG をカレントディレクトリに
`gazu-<hash>.svg` として書き出し、`Image` ノードとして埋め込む
（typst の `--root` はデフォルトで CWD なので追加オプションは不要）。
書き出された SVG は PDF 生成後も残るため、変換後は適宜削除してよい。

#### 対応 PDF engine

| PDF engine | 動作 |
|---|---|
| `weasyprint` | ✓ (HTML 経由) |
| `typst` | ✓ (SVG をファイル経由の `Image` に変換) |
| `pdflatex` / `xelatex` / `lualatex` | ✗ (graphicx が SVG を直接扱えない。`svg` パッケージ + Inkscape + `--shell-escape` が別途必要) |

## CLI オプション

| オプション | 説明 |
|---|---|
| `--version`, `-v` | バージョンを表示する |
| `--help`, `-h` | ヘルプを表示する |

## Mermaid の設定 (テーマ・フォント等)

`GAZU_CONFIG` 環境変数に JSON ファイルのパスを指定すると、
`mermaid.initialize()` に渡す設定をカスタマイズできる。
[mmdc](https://github.com/mermaid-js/mermaid-cli) の `--configFile` と同じ形式
（`mermaid.initialize()` に渡すオブジェクトそのもの）の JSON ファイルを使う:

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

## 動作概要

1. stdin から Pandoc AST (JSON) を読み、引数から出力フォーマット
   (`html`, `typst`, ...) を受け取る
2. `CodeBlock` のうち class に `mermaid` を持つものを深さ優先で収集する
   （Div・BlockQuote・リスト内のブロックも対象）
3. 収集した Mermaid を **一括** レンダリングする
4. per-block の結果を出力フォーマットに応じて AST に適用する:
   - 成功 + raw HTML を素通りするフォーマット (`html` 等) → `RawBlock("html", svg)` に置換
   - 成功 + それ以外 (`typst` 等) → SVG をファイルに書き出し `Image` に置換
   - 失敗 → 元の `CodeBlock` をそのまま残し、stderr に warning を出す
5. 加工した AST を stdout に書き出す

文書内の N 個の Mermaid ブロックを `render_stream` 一回呼び出しでまとめて
処理するため、Xvfb / WebView / mermaid.js の初期化コストは 1 回分のみ。

## vs mermaid-filter

[mermaid-filter](https://github.com/raghur/mermaid-filter) は同じく Pandoc filter
として動作する代表的な実装だが、Mermaid ブロック 1 個につき `mmdc`
(Puppeteer/Chromium) を 1 プロセス起動する。gazu は文書内の全ブロックを
`render_stream` 一回でまとめてレンダリングするため、ブロック数が増えても
起動コストは1回分のまま。

- Figures are the median of 5 runs (Linux x86_64), `util/bench/fixture.md`
  (3 Mermaid diagrams). mermaid-filter 1.4.x (mmdc 11.14.0) / gazu 0.1.0
  (mermaid.js 11.14.0, via sekien 0.3.0)
- 計測方法は `util/bench/bench.sh` を参照
- Speed / Memory はフィルタ本体 (gazu / mermaid-filter) とその子プロセス
  (Xvfb・WebKit・mmdc・Chromium 等) のみを計測したもので、`pandoc` 本体の
  AST 生成・出力フォーマット変換は含まない
- Install size は gazu は単一バイナリのサイズ、mermaid-filter は npm
  パッケージ + Puppeteer がダウンロードする Chromium 本体の合計

| Metric | gazu | mermaid-filter | Advantage |
|---|---|---|---|
| Install size | **5.0 MB** | ~568 MB | **99% smaller** |
| Speed (3 diagrams) | **~2.0 s** | ~14.8 s | **~7x faster** |
| Memory (RSS) | **~446 MB** | ~849 MB | **~47% less** |

```bash
./util/bench/bench.sh
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Bundled Assets

gazu は依存クレート [sekien](https://github.com/olibrauma/sekien) 経由で
`mermaid.js` をバイナリに静的に埋め込んでいる。

- `mermaid.js`: Licensed under the [MIT License](mermaid.LICENSE). Copyright (c) 2014 - 2024 Knut Sveidqvist and contributors.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
