#!/usr/bin/env bash
# Regenerates crates/app/res/pinwheel.ico and docs/images/logo.png from
# docs/images/logo.svg. Needs resvg and python3.
# The icon holds PNG images: icon.rs decodes them. 20 and 24 px are what the
# notification area asks for at 125 % and 150 % display scaling.
set -euo pipefail
cd "$(dirname "$0")/.."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
sizes="16 20 24 32 40 48 64 128 256"
for size in $sizes; do
  resvg -w "$size" -h "$size" docs/images/logo.svg "$tmp/$size.png"
done
cp "$tmp/256.png" docs/images/logo.png

python3 - "$tmp" crates/app/res/pinwheel.ico $sizes <<'PY'
import struct, sys
tmp, out, *sizes = sys.argv[1:]
pngs = [open(f"{tmp}/{size}.png", "rb").read() for size in sizes]
offset = 6 + 16 * len(pngs)
header, body = struct.pack("<HHH", 0, 1, len(pngs)), b""
for size, png in zip(map(int, sizes), pngs):
    # A size of 256 is stored as 0.
    header += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(png), offset + len(body))
    body += png
open(out, "wb").write(header + body)
PY
