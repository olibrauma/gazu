#!/bin/bash
# Checks or regenerates the src/filter.rs HTML_FORMATS allowlist by measurement.
#
# Usage:
#   util/check-html-formats.sh           — print PASS for each format that passes SVG through
#   util/check-html-formats.sh --check   — exit non-zero if HTML_FORMATS in src/filter.rs
#                                          is out of date for the installed pandoc version
#
# Runs a Pandoc AST containing RawBlock("html", "<svg>...</svg>") through
# each output format's writer, and lists as "PASS" the formats where the SVG
# content comes out as-is (or in the format's own raw-HTML syntax).
#
# When bumping the pandoc version, re-run without --check and revisit
# HTML_FORMATS / the comment in src/filter.rs.
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

passing=()
for f in $(pandoc --list-output-formats); do
    case " $skip " in
        *" $f "*) continue ;;
    esac

    out=$(pandoc -f json -t "$f" "$tmp" 2>/dev/null || true)
    if echo "$out" | grep -q "circle"; then
        passing+=("$f")
    fi
done

if [[ "${1:-}" == "--check" ]]; then
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    pandoc_rs="$script_dir/../src/filter.rs"

    actual=$(printf '%s\n' "${passing[@]}" | sort)
    expected=$(awk '/const HTML_FORMATS/,/^\];/' "$pandoc_rs" \
        | grep -oP '"[a-z][a-z0-9_]*"' | tr -d '"' | sort)

    if diff <(echo "$expected") <(echo "$actual") > /dev/null; then
        echo "HTML_FORMATS is up to date (pandoc $(pandoc --version | head -1))."
    else
        echo "HTML_FORMATS in src/filter.rs is out of date for $(pandoc --version | head -1)."
        echo ""
        echo "Diff (- expected in filter.rs, + actually measured):"
        diff <(echo "$expected") <(echo "$actual") || true
        echo ""
        echo "Re-run util/check-html-formats.sh (without --check) to see the current list."
        exit 1
    fi
else
    for f in "${passing[@]}"; do
        echo "PASS  $f"
    done
fi
