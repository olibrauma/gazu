#!/bin/bash
# Regenerates the src/pandoc.rs HTML_FORMATS allowlist by measurement.
#
# Runs a Pandoc AST containing RawBlock("html", "<svg>...</svg>") through
# each output format's writer, and lists as "PASS" the formats where the SVG
# content comes out as-is (or in the format's own raw-HTML syntax).
#
# When bumping the pandoc version, re-run this script and revisit
# HTML_FORMATS / the comment in src/pandoc.rs.
#
# Notes:
#   - chunkedhtml is excluded because it's a multi-file writer (requires
#     `-o <directory>`) and can't be checked via stdout this way.
#   - Binary formats like docx/odt/pptx/epub are also excluded (not text
#     output).

set -euo pipefail

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

cat > "$tmp" <<'EOF'
{"pandoc-api-version":[1,23],"meta":{},"blocks":[{"t":"RawBlock","c":["html","<svg><circle r=\"5\"/></svg>"]}]}
EOF

skip="docx odt pptx epub epub2 epub3 rtf icml fb2 pdf bibtex biblatex csljson json native ipynb chunkedhtml"

for f in $(pandoc --list-output-formats); do
    case " $skip " in
        *" $f "*) continue ;;
    esac

    out=$(pandoc -f json -t "$f" "$tmp" 2>/dev/null || true)
    if echo "$out" | grep -q "circle"; then
        echo "PASS  $f"
    fi
done
