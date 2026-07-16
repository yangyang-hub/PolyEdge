#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
target="${CARGO_BUILD_TARGET:-}"

cargo_args=(build --release -p polyedge-server)
if [[ -n "${target}" ]]; then
  cargo_args+=(--target "${target}")
fi

(
  cd "${repo_root}/packages/backend"
  cargo "${cargo_args[@]}"
)

if [[ -n "${target}" ]]; then
  source_bin="${repo_root}/packages/backend/target/${target}/release/polyedge-server"
else
  source_bin="${repo_root}/packages/backend/target/release/polyedge-server"
fi

[[ -f "${source_bin}" ]] || {
  printf 'built binary not found: %s\n' "${source_bin}" >&2
  exit 1
}

mkdir -p "${repo_root}/bin"
cp "${source_bin}" "${repo_root}/bin/polyedge-server"
chmod 0755 "${repo_root}/bin/polyedge-server"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${repo_root}/bin/polyedge-server"
else
  shasum -a 256 "${repo_root}/bin/polyedge-server"
fi
