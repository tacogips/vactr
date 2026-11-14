#!/usr/bin/env bash
set -u
cd "$(dirname "$0")/.."
list_file=$(mktemp)
trap 'rm -f "$list_file"' EXIT
if ! git -C .. ls-files -z '*.vact' > "$list_file"; then
  for file in ../examples/*.vact ../src/prelude/*.vact; do
    [ -f "$file" ] || continue
    printf '%s\0' "${file#../}"
  done > "$list_file"
fi
count=0
while IFS= read -r -d '' file; do
  count=$((count + 1))
  parse_path=$file
  case "$file" in
    /*) ;;
    *) parse_path="../$file" ;;
  esac
  output=$(tree-sitter parse --grammar-path . "$parse_path" 2>&1)
  rc=$?
  if [ "$rc" -ne 0 ] || printf '%s\n' "$output" | grep -Eq 'ERROR|MISSING'; then
    printf 'parse-all: failed: %s\n%s\n' "$file" "$output" >&2
    exit 1
  fi
done < "$list_file"
if [ "$count" -lt 10 ]; then
  printf 'parse-all: expected at least 10 files, found %s\n' "$count" >&2
  exit 1
fi
printf 'parse-all: %s files ok\n' "$count"
