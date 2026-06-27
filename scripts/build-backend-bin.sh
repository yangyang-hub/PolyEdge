#!/usr/bin/env bash
set -Eeuo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

target="${CARGO_BUILD_TARGET:-}"

if [[ -n "${POLYEDGE_BACKEND_PACKAGE:-}" || -n "${POLYEDGE_BACKEND_BINARY:-}" ]]; then
  selected_package="${POLYEDGE_BACKEND_PACKAGE:-${POLYEDGE_BACKEND_BINARY:-polyedge-api}}"
  packages=("${selected_package}")
  binaries=("${POLYEDGE_BACKEND_BINARY:-${selected_package}}")
else
  packages=("polyedge-api" "polyedge-orderbook")
  binaries=("polyedge-api" "polyedge-orderbook")
fi

cargo_args=(build --release)
for package in "${packages[@]}"; do
  cargo_args+=(-p "${package}")
done
if [[ -n "${target}" ]]; then
  cargo_args+=(--target "${target}")
fi

(
  cd "${repo_root}/packages/backend"
  cargo "${cargo_args[@]}"
)

mkdir -p "${repo_root}/bin"

for binary in "${binaries[@]}"; do
  if [[ -n "${target}" ]]; then
    source_bin="${repo_root}/packages/backend/target/${target}/release/${binary}"
  else
    source_bin="${repo_root}/packages/backend/target/release/${binary}"
  fi

  [[ -f "${source_bin}" ]] || {
    printf 'built binary not found: %s\n' "${source_bin}" >&2
    exit 1
  }

  cp "${source_bin}" "${repo_root}/bin/${binary}"
  chmod 0755 "${repo_root}/bin/${binary}"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${repo_root}/bin/${binary}"
  else
    shasum -a 256 "${repo_root}/bin/${binary}"
  fi
done
