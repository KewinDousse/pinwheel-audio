#!/usr/bin/env bash
# Builds target/x86_64-pc-windows-msvc/release/pinwheel.exe from Linux.
# Needs cargo-xwin and LLVM (clang-cl, lld-link, llvm-rc), see README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

# Homebrew installs LLVM and LLD keg-only: add them when present.
if command -v brew >/dev/null; then
  for formula in llvm lld; do
    prefix="$(brew --prefix "$formula" 2>/dev/null || true)"
    [[ -d "$prefix/bin" ]] && PATH="$prefix/bin:$PATH"
  done
fi
PATH="$HOME/.cargo/bin:$PATH"
export PATH

cargo xwin build -p pinwheel --release --target x86_64-pc-windows-msvc "$@"
echo "target/x86_64-pc-windows-msvc/release/pinwheel.exe"
