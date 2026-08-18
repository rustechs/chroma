#!/usr/bin/env bash
# Install chroma with the host Rust toolchain so ~/.cargo/bin/chroma uses the
# host dynamic linker. `nix develop -c cargo install` links against Nix glibc
# and fails outside the shell with "required file not found".
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

if [[ -n "${IN_NIX_SHELL:-}" ]]; then
  echo "install-host: refuse Nix shell; run this outside \`nix develop\`" >&2
  exit 1
fi

rustc_path="$(command -v rustc || true)"
if [[ -z "${rustc_path}" ]]; then
  echo "install-host: rustc not found on PATH" >&2
  exit 1
fi
rustc_resolved="$(readlink -f "${rustc_path}")"
if [[ "${rustc_resolved}" == /nix/store/* ]]; then
  echo "install-host: refuse Nix rustc (${rustc_resolved}); use host cargo" >&2
  exit 1
fi

jobs="$(($(nproc) - 1))"
if [[ "${jobs}" -lt 1 ]]; then
  jobs=1
fi

cargo install --path . --force -j "${jobs}"

bin="${CARGO_HOME:-${HOME}/.cargo}/bin/chroma"
if [[ ! -x "${bin}" ]]; then
  echo "install-host: missing installed binary: ${bin}" >&2
  exit 1
fi

interp="$(readelf -l "${bin}" | awk '/Requesting program interpreter:/ {print $NF}' | tr -d '[]')"
if [[ "${interp}" == /nix/store/* ]]; then
  echo "install-host: installed binary still uses Nix interpreter: ${interp}" >&2
  exit 1
fi

echo "install-host: installed ${bin} (interpreter ${interp})"
