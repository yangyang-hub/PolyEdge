#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

package="${POLYEDGE_BACKEND_PACKAGE:-polyedge-api}"
binary="${POLYEDGE_BACKEND_BINARY:-polyedge-api}"
target="${CARGO_BUILD_TARGET:-}"

cargo_args=(build --release -p "${package}")
if [[ -n "${target}" ]]; then
  cargo_args+=(--target "${target}")
fi

(
  cd "${repo_root}/packages/backend"
  cargo "${cargo_args[@]}"
)

if [[ -n "${target}" ]]; then
  source_bin="${repo_root}/packages/backend/target/${target}/release/${binary}"
else
  source_bin="${repo_root}/packages/backend/target/release/${binary}"
fi

[[ -f "${source_bin}" ]] || {
  printf 'built binary not found: %s\n' "${source_bin}" >&2
  exit 1
}

mkdir -p "${repo_root}/bin"
cp "${source_bin}" "${repo_root}/bin/${binary}"
chmod 0755 "${repo_root}/bin/${binary}"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "${repo_root}/bin/${binary}"
else
  shasum -a 256 "${repo_root}/bin/${binary}"
fi
