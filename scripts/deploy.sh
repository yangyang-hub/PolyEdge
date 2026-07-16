#!/usr/bin/env bash
set -Eeuo pipefail

# PolyEdge single-backend deployment.
#
# Usage:
#   scripts/deploy.sh              # auto: pull and deploy changed services
#   scripts/deploy.sh auto
#   scripts/deploy.sh all
#   scripts/deploy.sh server
#   scripts/deploy.sh front
#   scripts/deploy.sh server,front
#
# Environment:
#   POLYEDGE_DEPLOY_DIR              repository checkout
#   POLYEDGE_GIT_REPO                remote URL used for first clone
#   POLYEDGE_GIT_BRANCH              branch to track
#   POLYEDGE_SKIP_GIT_PULL=1         do not fetch/fast-forward
#   POLYEDGE_COMPOSE_FILE            compose file override
#   POLYEDGE_SERVER_ENV_FILE         server env file override
#   POLYEDGE_FRONT_ENV_FILE          frontend env file override
#   POLYEDGE_SKIP_ENV_VALIDATION=1   skip configuration validation
#   POLYEDGE_SKIP_SERVICES           comma-separated server/front exclusions
#   POLYEDGE_LOG_FILE                deployment log path
#   POLYEDGE_DEPLOY_LOCK_FILE        non-overlap lock file

log() {
  printf '[polyedge-deploy] %s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"
}

fail() {
  printf '[polyedge-deploy] ERROR: %s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >&2
  exit 1
}

usage() {
  cat >&2 <<'EOF'
Usage: scripts/deploy.sh [auto|all|server|front] [...]

Targets:
  no args / auto  Pull code and deploy only changed or stopped services.
  all             Rebuild and restart polyedge-server and polyedge-front.
  server          Rebuild and restart the single Rust backend.
  front           Rebuild and restart the static frontend.

Targets may be separate or comma-separated, for example:
  scripts/deploy.sh server front
  scripts/deploy.sh server,front
EOF
}

mode="auto"

parse_targets() {
  local -n target_server_ref="$1"
  local -n target_front_ref="$2"
  local raw part target
  local -a parts
  shift 2

  [[ $# -gt 0 ]] || return 0
  for raw in "$@"; do
    IFS=',' read -r -a parts <<< "${raw}"
    for part in "${parts[@]}"; do
      target="${part,,}"
      case "${target}" in
        auto)
          mode="auto"
          ;;
        all)
          mode="manual"
          target_server_ref=1
          target_front_ref=1
          ;;
        server)
          mode="manual"
          target_server_ref=1
          ;;
        front)
          mode="manual"
          target_front_ref=1
          ;;
        ""|-h|--help|help)
          usage
          exit 0
          ;;
        *)
          usage
          fail "unknown deploy target: ${part}. Expected auto, all, server, or front."
          ;;
      esac
    done
  done
}

set_compose_command() {
  if docker compose version >/dev/null 2>&1; then
    compose_cmd=(docker compose)
    return 0
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    compose_cmd=(docker-compose)
    return 0
  fi
  return 1
}

env_value() {
  local key="$1"
  local file="$2"
  local line value
  line="$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "${file}" | tail -n1 || true)"
  [[ -n "${line}" ]] || return 0
  value="${line#*=}"
  value="$(printf '%s' "${value}" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
  value="${value%\"}"
  value="${value#\"}"
  value="${value%\'}"
  value="${value#\'}"
  printf '%s' "${value}"
}

env_truthy() {
  local value="${1:-}"
  value="${value,,}"
  [[ "${value}" == "1" || "${value}" == "true" || "${value}" == "yes" || "${value}" == "on" ]]
}

ensure_env_file() {
  local file="$1"
  local example="$2"
  local label="$3"
  local -n created_ref="$4"
  [[ -f "${file}" ]] && return 0
  [[ -f "${example}" ]] || fail "${label} env example not found: ${example}"
  cp "${example}" "${file}"
  created_ref+=("${file}")
}

validate_server_env_file() {
  local file="$1"
  [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" != "1" ]] || {
    log "skipping server env validation because POLYEDGE_SKIP_ENV_VALIDATION=1"
    return 0
  }
  [[ -f "${file}" ]] || fail "server env file not found: ${file}"

  local postgres_url runtime_environment auth_disabled auth_api_token
  local insecure_ack cors_allowed_origins step_up_code secret_prefix origin
  local -a origins
  postgres_url="$(env_value POLYEDGE_POSTGRES__URL "${file}")"
  [[ -n "${postgres_url}" ]] || fail "POLYEDGE_POSTGRES__URL is required in ${file}."
  [[ "${postgres_url}" != *change-me* ]] || fail "POLYEDGE_POSTGRES__URL still contains change-me in ${file}."

  runtime_environment="$(env_value POLYEDGE_RUNTIME__ENVIRONMENT "${file}")"
  runtime_environment="${runtime_environment:-local}"
  auth_disabled="$(env_value POLYEDGE_AUTH__DISABLED "${file}")"
  if ! env_truthy "${auth_disabled}"; then
    auth_api_token="$(env_value POLYEDGE_AUTH__API_TOKEN "${file}")"
    if [[ -z "${auth_api_token}" || "${auth_api_token}" == *"replace-with"* || ${#auth_api_token} -lt 32 ]]; then
      fail "POLYEDGE_AUTH__API_TOKEN must contain a real token of at least 32 characters when authentication is enabled (${file})."
    fi
  fi
  if [[ "${runtime_environment}" == "production" ]] && env_truthy "${auth_disabled}"; then
    insecure_ack="$(env_value POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY "${file}")"
    env_truthy "${insecure_ack}" || fail "POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true is required when production authentication is disabled (${file})."
  fi
  step_up_code="$(env_value POLYEDGE_AUTH__STEP_UP_CODE "${file}")"
  if [[ "${runtime_environment}" == "production" ]]; then
    if [[ -z "${step_up_code}" || "${step_up_code}" == *"replace-with"* || ${#step_up_code} -lt 16 ]]; then
      fail "POLYEDGE_AUTH__STEP_UP_CODE must contain a real value of at least 16 characters in production (${file})."
    fi
  fi

  cors_allowed_origins="$(env_value POLYEDGE_CORS__ALLOWED_ORIGINS "${file}")"
  if [[ "${runtime_environment}" == "production" && -z "${cors_allowed_origins}" ]]; then
    fail "POLYEDGE_CORS__ALLOWED_ORIGINS must contain at least one exact frontend origin in production (${file})."
  fi
  [[ "${cors_allowed_origins}" != *"*"* ]] || fail "POLYEDGE_CORS__ALLOWED_ORIGINS must not contain wildcard origins in ${file}."
  if [[ -n "${cors_allowed_origins}" ]]; then
    IFS=',' read -r -a origins <<< "${cors_allowed_origins}"
    for origin in "${origins[@]}"; do
      origin="$(printf '%s' "${origin}" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
      [[ "${origin}" =~ ^https?://[^/?#]+$ ]] || fail "invalid CORS origin '${origin}' in ${file}; use scheme + host + optional port only."
    done
  fi

  secret_prefix="$(env_value POLYEDGE_WALLET_SECRETS__ENV_PREFIX "${file}")"
  [[ -n "${secret_prefix}" ]] || fail "POLYEDGE_WALLET_SECRETS__ENV_PREFIX must be set in ${file}."
}

validate_front_env_file() {
  local file="$1"
  [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" != "1" ]] || return 0
  [[ -f "${file}" ]] || fail "frontend env file not found: ${file}"
  local api_base_url console_auth
  api_base_url="$(env_value NEXT_PUBLIC_POLYEDGE_API_BASE_URL "${file}")"
  [[ -n "${api_base_url}" ]] || fail "NEXT_PUBLIC_POLYEDGE_API_BASE_URL must be set in ${file}."
  console_auth="$(env_value NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH "${file}")"
  [[ -z "${console_auth}" || "${console_auth}" == "off" ]] || fail "NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH currently supports only off (${file})."
}

export_env_if_set() {
  local file="$1"
  local key="$2"
  local value
  value="$(env_value "${key}" "${file}")"
  [[ -z "${value}" ]] || export "${key}=${value}"
}

load_compose_environment() {
  if [[ -f "${server_env_file}" ]]; then
    export POLYEDGE_SERVER_ENV_FILE="${server_env_file}"
    export_env_if_set "${server_env_file}" POLYEDGE_SERVER_IMAGE
    export_env_if_set "${server_env_file}" POLYEDGE_SERVER_BIND
    export_env_if_set "${server_env_file}" POLYEDGE_SERVER_PORT
  else
    export POLYEDGE_SERVER_ENV_FILE="${deploy_path}/.env.server.example"
  fi
  if [[ -f "${front_env_file}" ]]; then
    export POLYEDGE_FRONT_ENV_FILE="${front_env_file}"
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_IMAGE
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_BIND
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_PORT
  else
    export POLYEDGE_FRONT_ENV_FILE="${deploy_path}/.env.front.example"
  fi
}

file_hash() {
  if [[ -f "$1" ]]; then
    md5sum "$1" 2>/dev/null | awk '{print $1}'
  else
    printf 'MISSING'
  fi
}

combined_file_hash() {
  local file
  for file in "$@"; do
    printf '%s %s\n' "${file}" "$(file_hash "${file}")"
  done | md5sum | awk '{print $1}'
}

frontend_hash() {
  local path="$1"
  if [[ -d "${path}" ]]; then
    find "${path}" \
      \( -path '*/node_modules' -o -path '*/.next' -o -path '*/out' -o -path '*/dist' -o -path '*/coverage' -o -path '*/.turbo' \) -prune \
      -o -type f -not -name '*.tsbuildinfo' -print0 \
      | sort -z | xargs -0 md5sum 2>/dev/null | md5sum | awk '{print $1}'
  else
    printf 'MISSING'
  fi
}

frontend_build_hash() {
  printf '%s\n%s\n' "$(frontend_hash "$1")" "$(file_hash "$2")" | md5sum | awk '{print $1}'
}

load_frontend_build_env() {
  validate_front_env_file "$1"
  export NEXT_PUBLIC_POLYEDGE_API_BASE_URL="$(env_value NEXT_PUBLIC_POLYEDGE_API_BASE_URL "$1")"
  local value
  value="$(env_value NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS "$1")"
  [[ -z "${value}" ]] || export NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS="${value}"
  value="$(env_value NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH "$1")"
  [[ -z "${value}" ]] || export NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH="${value}"
  log "frontend build env loaded from $1: NEXT_PUBLIC_POLYEDGE_API_BASE_URL=${NEXT_PUBLIC_POLYEDGE_API_BASE_URL}"
}

version_frontend_static_assets() {
  local front_dir="$1"
  local asset_version="$2"
  log "versioning frontend static asset references (${asset_version:0:12})"
  FRONT_OUT_DIR="${front_dir}/out" FRONT_ASSET_VERSION="${asset_version}" node <<'NODE'
const fs = require("fs");
const path = require("path");
const outDir = process.env.FRONT_OUT_DIR;
const version = process.env.FRONT_ASSET_VERSION;
if (!outDir || !version) process.exit(1);
function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(fullPath, files);
    else if (entry.isFile() && entry.name.endsWith(".html")) files.push(fullPath);
  }
  return files;
}
const pattern = /(\/_next\/static\/[^"'<>\\\s]+?\.(?:js|css))(?:\?v=[0-9a-f]+)?/g;
for (const file of walk(outDir)) {
  const current = fs.readFileSync(file, "utf8");
  const next = current.replace(pattern, `$1?v=${version}`);
  if (next !== current) fs.writeFileSync(file, next);
}
NODE
}

build_frontend() {
  local front_dir="${deploy_dir}/packages/front"
  local asset_version
  [[ -f "${front_dir}/package.json" ]] || fail "packages/front/package.json not found"
  load_frontend_build_env "${front_env_file}"
  asset_version="$(frontend_build_hash "${front_dir}" "${front_env_file}")"
  log "building frontend static files (yarn build)"
  (cd "${front_dir}" && rm -rf .next out && yarn install --frozen-lockfile && yarn build) || fail "frontend yarn build failed"
  [[ -d "${front_dir}/out" ]] || fail "frontend build did not produce out/"
  version_frontend_static_assets "${front_dir}" "${asset_version}"
}

container_running() {
  local service="$1"
  local status
  status="$("${compose_cmd[@]}" -f "${compose_file}" ps --format json "${service}" 2>/dev/null)" || true
  [[ -n "${status}" ]] && printf '%s' "${status}" | tail -1 | grep -q '"running"'
}

should_skip_service() {
  local service="$1"
  local skip="${POLYEDGE_SKIP_SERVICES:-}"
  local item
  [[ -n "${skip}" ]] || return 1
  local IFS=','
  for item in ${skip}; do
    item="${item,,}"
    item="${item#polyedge-}"
    [[ "${item}" != "${service}" ]] || return 0
  done
  return 1
}

save_deploy_state() {
  local state_file="$1"
  {
    printf 'server_hash=%s\n' "${current_server_hash}"
    printf 'front_hash=%s\n' "${current_front_hash}"
    printf 'commit=%s\n' "$(git rev-parse HEAD 2>/dev/null || printf unknown)"
    printf 'deployed_at=%s\n' "$(date '+%Y-%m-%d %H:%M:%S')"
  } > "${state_file}"
  log "deploy state saved to ${state_file}"
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_root="$(cd "${script_dir}/.." && pwd)"
deploy_dir="${POLYEDGE_DEPLOY_DIR:-${default_root}}"
repo_url="${POLYEDGE_GIT_REPO:-}"
branch="${POLYEDGE_GIT_BRANCH:-}"
skip_git_pull="${POLYEDGE_SKIP_GIT_PULL:-0}"
target_server=0
target_front=0
parse_targets target_server target_front "$@"

log_file="${POLYEDGE_LOG_FILE:-}"
if [[ -z "${log_file}" && ! -t 0 ]]; then
  log_file="${HOME:-/tmp}/polyedge-deploy.log"
fi
if [[ -n "${log_file}" ]] && mkdir -p "$(dirname "${log_file}")" && touch "${log_file}" 2>/dev/null; then
  exec > >(tee -a "${log_file}") 2>&1
fi

lock_file="${POLYEDGE_DEPLOY_LOCK_FILE:-/tmp/polyedge-deploy.lock}"
if command -v flock >/dev/null 2>&1; then
  exec 9>"${lock_file}"
  flock -n 9 || { log "another deploy is running; skip (${lock_file})"; exit 0; }
fi

log "=== deploy start (mode=${mode}) ==="
if [[ ! -d "${deploy_dir}/.git" ]]; then
  [[ -n "${repo_url}" ]] || fail "POLYEDGE_DEPLOY_DIR is not a git checkout; set POLYEDGE_GIT_REPO for first clone."
  [[ -n "${branch}" ]] || branch="main"
  if [[ -e "${deploy_dir}" && -n "$(find "${deploy_dir}" -mindepth 1 -maxdepth 1 2>/dev/null)" ]]; then
    fail "POLYEDGE_DEPLOY_DIR exists and is not empty: ${deploy_dir}"
  fi
  mkdir -p "$(dirname "${deploy_dir}")"
  git clone --branch "${branch}" "${repo_url}" "${deploy_dir}"
fi

cd "${deploy_dir}"
if [[ -z "${branch}" ]]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  [[ "${branch}" != "HEAD" ]] || branch="main"
fi

new_code=0
pre_merge_head=""
if [[ "${skip_git_pull}" != "1" ]]; then
  [[ -z "$(git status --porcelain --untracked-files=no)" ]] || fail "tracked files have local changes; commit, stash, or set POLYEDGE_SKIP_GIT_PULL=1."
  git fetch --prune origin "${branch}"
  current_branch="$(git rev-parse --abbrev-ref HEAD)"
  if [[ "${current_branch}" != "${branch}" ]]; then
    git show-ref --verify --quiet "refs/heads/${branch}" && git checkout "${branch}" || git checkout -B "${branch}" "origin/${branch}"
  fi
  local_head="$(git rev-parse HEAD)"
  remote_head="$(git rev-parse "origin/${branch}")"
  if [[ "${local_head}" != "${remote_head}" ]]; then
    pre_merge_head="${local_head}"
    git merge --ff-only "origin/${branch}"
    new_code=1
  fi
else
  log "skipping git update"
fi

compose_file="${POLYEDGE_COMPOSE_FILE:-${deploy_dir}/deploy/docker-compose.yml}"
deploy_path="${deploy_dir}/deploy"
server_env_file="${POLYEDGE_SERVER_ENV_FILE:-${deploy_path}/.env.server}"
front_env_file="${POLYEDGE_FRONT_ENV_FILE:-${deploy_path}/.env.front}"
[[ -f "${compose_file}" ]] || fail "compose file not found: ${compose_file}"

created_env_files=()
if [[ "${mode}" == "auto" ]]; then
  should_skip_service server || ensure_env_file "${server_env_file}" "${deploy_path}/.env.server.example" server created_env_files
  should_skip_service front || ensure_env_file "${front_env_file}" "${deploy_path}/.env.front.example" front created_env_files
else
  [[ "${target_server}" != "1" ]] || ensure_env_file "${server_env_file}" "${deploy_path}/.env.server.example" server created_env_files
  [[ "${target_front}" != "1" ]] || ensure_env_file "${front_env_file}" "${deploy_path}/.env.front.example" front created_env_files
fi
if [[ ${#created_env_files[@]} -gt 0 ]]; then
  fail "created env file(s): ${created_env_files[*]}. Configure PostgreSQL, CORS/auth, wallet secret resolver, and frontend API URL, then rerun."
fi

if [[ "${mode}" == "auto" ]]; then
  should_skip_service server || validate_server_env_file "${server_env_file}"
  should_skip_service front || validate_front_env_file "${front_env_file}"
else
  [[ "${target_server}" != "1" ]] || validate_server_env_file "${server_env_file}"
  [[ "${target_front}" != "1" ]] || validate_front_env_file "${front_env_file}"
fi

set_compose_command || fail "Docker Compose is not installed."
export COMPOSE_PARALLEL_LIMIT="${COMPOSE_PARALLEL_LIMIT:-1}"
load_compose_environment

if [[ "${mode}" == "auto" ]]; then
  current_server_hash="$(combined_file_hash bin/polyedge-server deploy/server.Dockerfile deploy/docker-compose.yml "${server_env_file}")"
  current_front_hash="$(frontend_build_hash packages/front "${front_env_file}")"
  state_file="${deploy_dir}/.deploy-state"
  saved_server_hash=""
  saved_front_hash=""
  if [[ -f "${state_file}" ]]; then
    saved_server_hash="$(grep '^server_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_front_hash="$(grep '^front_hash=' "${state_file}" | cut -d= -f2 || true)"
  fi

  server_changed=0
  front_changed=0
  [[ "${current_server_hash}" == "${saved_server_hash}" ]] || server_changed=1
  [[ "${current_front_hash}" == "${saved_front_hash}" ]] || front_changed=1
  if [[ "${new_code}" == "1" && -n "${pre_merge_head}" ]]; then
    git diff --name-only "${pre_merge_head}" HEAD -- deploy/docker-compose.yml deploy/server.Dockerfile scripts/deploy.sh | grep -q . && server_changed=1
    git diff --name-only "${pre_merge_head}" HEAD -- packages/front/ deploy/.env.front.example | grep -q . && front_changed=1
  fi

  server_running=1
  front_running=1
  should_skip_service server || container_running polyedge-server || server_running=0
  should_skip_service front || container_running polyedge-front || front_running=0
  build_images=()
  restart_services=()

  if ! should_skip_service server && [[ "${server_changed}" == "1" || "${server_running}" == "0" ]]; then
    [[ -f bin/polyedge-server ]] || fail "bin/polyedge-server is missing; run scripts/build-backend-bin.sh."
    [[ "${server_changed}" != "1" ]] || build_images+=(polyedge-server)
    restart_services+=(polyedge-server)
  fi
  if ! should_skip_service front && [[ "${front_changed}" == "1" || "${front_running}" == "0" ]]; then
    [[ "${front_changed}" != "1" ]] || build_images+=(polyedge-front)
    restart_services+=(polyedge-front)
  fi

  if [[ ${#build_images[@]} -eq 0 && ${#restart_services[@]} -eq 0 ]]; then
    log "no changes detected and all targeted containers are running"
    exit 0
  fi
  if printf '%s\n' "${build_images[@]}" | grep -qx polyedge-front 2>/dev/null; then
    build_frontend
  fi
  if [[ ${#build_images[@]} -gt 0 ]]; then
    "${compose_cmd[@]}" -f "${compose_file}" build --pull "${build_images[@]}"
  fi
  if [[ ${#restart_services[@]} -gt 0 ]]; then
    "${compose_cmd[@]}" -f "${compose_file}" up -d --remove-orphans "${restart_services[@]}"
  fi
  save_deploy_state "${state_file}"
else
  build_images=()
  runtime_services=()
  if [[ "${target_server}" == "1" ]]; then
    [[ -f bin/polyedge-server ]] || fail "bin/polyedge-server is missing; run scripts/build-backend-bin.sh."
    build_images+=(polyedge-server)
    runtime_services+=(polyedge-server)
  fi
  if [[ "${target_front}" == "1" ]]; then
    build_images+=(polyedge-front)
    runtime_services+=(polyedge-front)
    build_frontend
  fi
  [[ ${#build_images[@]} -eq 0 ]] || "${compose_cmd[@]}" -f "${compose_file}" build --pull "${build_images[@]}"
  [[ ${#runtime_services[@]} -eq 0 ]] || "${compose_cmd[@]}" -f "${compose_file}" up -d --remove-orphans "${runtime_services[@]}"
fi

"${compose_cmd[@]}" -f "${compose_file}" ps
log "=== deploy end ==="
