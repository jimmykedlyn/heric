#!/usr/bin/env bash
# Fails when the three documentation languages diverge:
#   - docs/book/{en,pt-BR,es}/src must contain the same file names;
#   - the src/SUMMARY.md files must list the same paths in the same order and
#     with the same heading levels;
#   - README and CONTRIBUTING must have the same heading structure
#     (levels and counts), since the titles themselves are translated.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
status=0

fail() {
  echo "::error::$1"
  status=1
}

languages=(en pt-BR es)

# --- Book files ------------------------------------------------------------
list_files() {
  local dir="$1"
  (cd "$dir" && find . -type f -name '*.md' | sed 's|^\./||' | sort)
}

reference="$(list_files "$root/docs/book/en/src")"
for lang in "${languages[@]:1}"; do
  other="$(list_files "$root/docs/book/$lang/src")"
  if [ "$reference" != "$other" ]; then
    echo "--- docs/book/en/src vs docs/book/$lang/src"
    diff <(echo "$reference") <(echo "$other") || true
    fail "docs/book/$lang/src has a different file set from docs/book/en/src"
  fi
done

# --- SUMMARY.md paths and levels -------------------------------------------
summary_shape() {
  grep -oE '^[[:space:]]*[-*] \[[^]]*\]\([^)]*\)' "$1" |
    sed -E 's/^([[:space:]]*)[-*].*\(([^)]*)\)/\1|\2/'
}

reference_summary="$(summary_shape "$root/docs/book/en/src/SUMMARY.md")"
for lang in "${languages[@]:1}"; do
  other_summary="$(summary_shape "$root/docs/book/$lang/src/SUMMARY.md")"
  if [ "$reference_summary" != "$other_summary" ]; then
    fail "docs/book/$lang/src/SUMMARY.md differs in structure from the English one"
  fi
done

# --- Root documents --------------------------------------------------------
heading_shape() {
  grep -E '^#{1,6} ' "$1" | sed -E 's/^(#+).*/\1/'
}

for base in README CONTRIBUTING; do
  ref="$root/${base}.md"
  ref_shape="$(heading_shape "$ref")"
  for lang in "${languages[@]:1}"; do
    other="$root/${base}.${lang}.md"
    if [ ! -f "$other" ]; then
      fail "missing ${base}.${lang}.md"
      continue
    fi
    if [ "$ref_shape" != "$(heading_shape "$other")" ]; then
      fail "${base}.${lang}.md heading structure differs from ${base}.md"
    fi
  done
done

# --- Language selectors ----------------------------------------------------
check_selector() {
  local file="$1"
  local expected="$2"
  head -n 2 "$file" | grep -qF "English" || fail "$file is missing the language selector"
  # shellcheck disable=SC2086
  for target in $expected; do
    grep -qF "$target" "$file" || fail "$file selector misses $target"
  done
}

check_selector "$root/README.md" "README.pt-BR.md README.es.md"
check_selector "$root/README.pt-BR.md" "README.md README.es.md"
check_selector "$root/README.es.md" "README.md README.pt-BR.md"
check_selector "$root/CONTRIBUTING.md" "CONTRIBUTING.pt-BR.md CONTRIBUTING.es.md"
check_selector "$root/CONTRIBUTING.pt-BR.md" "CONTRIBUTING.md CONTRIBUTING.es.md"
check_selector "$root/CONTRIBUTING.es.md" "CONTRIBUTING.md CONTRIBUTING.pt-BR.md"

if [ "$status" -ne 0 ]; then
  echo "Documentation structure check failed."
  exit 1
fi

echo "Documentation structure check passed."
