#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="${SRC_DIR:-/Users/addmaple/sites/wasm-fast-compress/test-data/silesia}"
OUT_DIR="${OUT_DIR:-${ROOT_DIR}/bench-data}"
TEXT_SRC="${SRC_DIR}/mozilla"
JSON_SRC="${SRC_DIR}/large.json"
TEXT_OUT="${OUT_DIR}/text_50kb.txt"
JSON_OUT="${OUT_DIR}/json_50kb.json"
MANIFEST_OUT="${OUT_DIR}/MANIFEST.sha256"
BYTES=51200

sha256_file() {
  local path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
    return
  fi
  shasum -a 256 "$path" | awk '{print $1}'
}

for src in "$TEXT_SRC" "$JSON_SRC"; do
  if [[ ! -f "$src" ]]; then
    echo "missing source fixture: $src" >&2
    exit 1
  fi
done

mkdir -p "$OUT_DIR"

head -c "$BYTES" "$TEXT_SRC" > "$TEXT_OUT"
head -c "$BYTES" "$JSON_SRC" > "$JSON_OUT"

for out in "$TEXT_OUT" "$JSON_OUT"; do
  actual="$(wc -c < "$out" | tr -d ' ')"
  if [[ "$actual" != "$BYTES" ]]; then
    echo "fixture size mismatch for $out: expected $BYTES got $actual" >&2
    exit 1
  fi
done

text_hash="$(sha256_file "$TEXT_OUT")"
json_hash="$(sha256_file "$JSON_OUT")"
{
  echo "${text_hash}  text_50kb.txt"
  echo "${json_hash}  json_50kb.json"
} > "$MANIFEST_OUT"

echo "prepared fixtures in $OUT_DIR"
echo "text_50kb.txt  size=$BYTES sha256=$text_hash"
echo "json_50kb.json size=$BYTES sha256=$json_hash"
