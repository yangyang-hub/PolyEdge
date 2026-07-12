#!/usr/bin/env bash
set -Eeuo pipefail

# ---------------------------------------------------------------------------
# PolyEdge deploy script
#
# Usage:
#   scripts/deploy.sh                 # auto mode (default for cron/CI)
#   scripts/deploy.sh auto            # same as no args
#   scripts/deploy.sh all             # force rebuild everything
#   scripts/deploy.sh api [orderbook|front ...]
#
# Each service is deployed independently with its own image — orderbook, api,
# and front can run on different servers without cross-dependencies. The API
# binary embeds the worker runtime and loads its task settings from .env.api.
# Only the binaries required for the targeted services need to exist locally.
#
# Auto mode (default):
#   1. git fetch + fast-forward
#   2. If no changes detected AND all targeted containers are running -> skip
#   3. If api binary changed -> rebuild api image -> restart api
#   4. If orderbook binary changed -> rebuild orderbook image -> restart orderbook
#   5. If frontend files changed -> rebuild frontend image -> restart front
#   6. If any targeted container is not running -> start existing image
#
# Environment variables:
#   POLYEDGE_DEPLOY_DIR       - repo checkout (default: script's parent)
#   POLYEDGE_GIT_REPO         - remote URL (only used for first clone)
#   POLYEDGE_GIT_BRANCH       - branch to track (default: current)
#   POLYEDGE_COMPOSE_FILE     - docker-compose file path
#   POLYEDGE_API_ENV_FILE     - api env file path
#   POLYEDGE_ORDERBOOK_ENV_FILE - orderbook env file path
#   POLYEDGE_FRONT_ENV_FILE   - frontend env file path
#   POLYEDGE_SKIP_ENV_VALIDATION=1 - skip env sanity checks
#   POLYEDGE_LOG_FILE         - log file path (default for cron: $HOME/polyedge-deploy.log)
#   POLYEDGE_DEPLOY_LOCK_FILE - non-overlap lock file (default: /tmp/polyedge-deploy.lock)
#   POLYEDGE_SKIP_SERVICES    - comma-separated services to exclude (e.g. "orderbook,front")
#   COMPOSE_PARALLEL_LIMIT    - compose build parallelism (default: 1)
# ---------------------------------------------------------------------------

log() {
  printf '[polyedge-deploy] %s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"
}

fail() {
  printf '[polyedge-deploy] ERROR: %s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >&2
  exit 1
}

usage() {
  cat >&2 <<'EOF'
Usage: scripts/deploy.sh [auto|all|orderbook|api|worker|front] [...]

Targets:
  no args / auto  Intelligent deploy: pull code, detect changes, deploy only what changed.
  all             Force rebuild all available images, then restart all available services.
  orderbook       Rebuild the orderbook image and restart only the orderbook service.
  api             Rebuild the API image and restart API plus embedded worker tasks.
  worker          Compatibility alias for api.
  front           Rebuild the frontend image and restart only the frontend service.

Each service deploys independently. Only the binaries for targeted services need to
exist locally. Set POLYEDGE_SKIP_SERVICES to exclude services (e.g. "orderbook").

Multiple targets can be passed as separate args or comma-separated, for example:
  scripts/deploy.sh worker
  scripts/deploy.sh api,front
EOF
}

# mode: "auto" (change-detection) or "manual" (explicit targets)
mode="auto"

parse_targets() {
  local -n target_api_ref="$1"
  local -n target_front_ref="$2"
  local -n target_orderbook_ref="$3"
  local raw
  local target
  local part
  local -a parts

  shift 3

  if [[ $# -eq 0 ]]; then
    return 0
  fi

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
          # Mark all as targeted; missing binaries will be skipped gracefully.
          target_api_ref=1
          target_front_ref=1
          target_orderbook_ref=1
          ;;
        api)
          mode="manual"
          target_api_ref=1
          ;;
        worker)
          mode="manual"
          target_api_ref=1
          ;;
        orderbook|ob)
          mode="manual"
          target_orderbook_ref=1
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
          fail "unknown deploy target: ${part}. Expected auto, all, api, worker, orderbook, or front."
          ;;
      esac
    done
  done
}

find_compose() {
  if docker compose version >/dev/null 2>&1; then
    printf 'docker compose'
    return 0
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    printf 'docker-compose'
    return 0
  fi

  return 1
}

env_value() {
  local key="$1"
  local file="$2"
  local line
  line="$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "${file}" | tail -n1 || true)"
  [[ -n "${line}" ]] || return 0

  local value="${line#*=}"
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

validate_postgres_env_file() {
  local file="$1"
  local service="$2"

  if [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" == "1" ]]; then
    log "skipping env validation because POLYEDGE_SKIP_ENV_VALIDATION=1"
    return 0
  fi
  [[ -f "${file}" ]] || fail "${service} env file not found: ${file}"

  local postgres_url
  postgres_url="$(env_value POLYEDGE_POSTGRES__URL "${file}")"
  local allow_in_memory
  allow_in_memory="$(env_value POLYEDGE_ALLOW_IN_MEMORY_DEPLOY "${file}")"
  if [[ -z "${postgres_url}" && "${allow_in_memory}" != "1" ]]; then
    fail "POLYEDGE_POSTGRES__URL is empty in ${file}. Production deploys require PostgreSQL; set POLYEDGE_ALLOW_IN_MEMORY_DEPLOY=1 only for throwaway demos."
  fi
  if [[ "${postgres_url}" == *change-me* ]]; then
    fail "POLYEDGE_POSTGRES__URL still contains change-me in ${file}."
  fi
}

validate_api_env_file() {
  local file="$1"
  if [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" == "1" ]]; then
    log "skipping api env validation because POLYEDGE_SKIP_ENV_VALIDATION=1"
    return 0
  fi

  validate_postgres_env_file "${file}" "api"

  local auth_disabled
  auth_disabled="$(env_value POLYEDGE_AUTH__DISABLED "${file}")"
  if ! env_truthy "${auth_disabled}"; then
    local auth_keys_json
    auth_keys_json="$(env_value POLYEDGE_AUTH__KEYS_JSON "${file}")"
    if [[ -z "${auth_keys_json}" || "${auth_keys_json}" == "[]" || "${auth_keys_json}" == *"<base64"* ]]; then
      fail "POLYEDGE_AUTH__KEYS_JSON must contain at least one real Ed25519 public key in ${file} when POLYEDGE_AUTH__DISABLED=false. The current static frontend also requires an external short-lived JWT issuance/transport integration."
    fi
  fi

  local runtime_environment
  runtime_environment="$(env_value POLYEDGE_RUNTIME__ENVIRONMENT "${file}")"
  runtime_environment="${runtime_environment:-local}"
  if [[ "${runtime_environment}" == "production" ]] && env_truthy "${auth_disabled}"; then
    local insecure_private_deploy_ack
    insecure_private_deploy_ack="$(env_value POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY "${file}")"
    if ! env_truthy "${insecure_private_deploy_ack}"; then
      fail "POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true is required in ${file} when production authentication is disabled; expose the API only behind a VPN, private ACL, or trusted access proxy."
    fi
  fi
  local cors_allowed_origins
  cors_allowed_origins="$(env_value POLYEDGE_CORS__ALLOWED_ORIGINS "${file}")"
  if [[ "${runtime_environment}" == "production" && -z "${cors_allowed_origins}" ]]; then
    fail "POLYEDGE_CORS__ALLOWED_ORIGINS must contain at least one exact frontend origin in production (${file})."
  fi
  if [[ "${cors_allowed_origins}" == *"*"* ]]; then
    fail "POLYEDGE_CORS__ALLOWED_ORIGINS must not contain wildcard origins in ${file}."
  fi
  local dev_bypass
  dev_bypass="$(env_value POLYEDGE_INTERNAL_AUTH_DEV_BYPASS "${file}")"
  dev_bypass="${dev_bypass:-0}"
  if ! env_truthy "${auth_disabled}" && [[ "${runtime_environment}" != "local" && "${dev_bypass}" == "1" ]]; then
    fail "POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1 is only allowed with POLYEDGE_RUNTIME__ENVIRONMENT=local."
  fi

  local orderbook_service_url
  orderbook_service_url="$(env_value POLYEDGE_ORDERBOOK__SERVICE_URL "${file}")"
  if [[ -z "${orderbook_service_url}" ]]; then
    fail "POLYEDGE_ORDERBOOK__SERVICE_URL must be set in ${file}."
  fi

  validate_orderbook_write_token "${file}" "api embedded worker runtime"
}

validate_orderbook_write_token() {
  local file="$1"
  local service="$2"
  if [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" == "1" ]]; then
    return 0
  fi
  [[ -f "${file}" ]] || fail "${service} env file not found: ${file}"

  local orderbook_write_token
  orderbook_write_token="$(env_value POLYEDGE_ORDERBOOK__WRITE_TOKEN "${file}")"
  if [[ -z "${orderbook_write_token}" || "${orderbook_write_token}" == *change-me* ]]; then
    fail "POLYEDGE_ORDERBOOK__WRITE_TOKEN must be set to a non-placeholder value in ${file} for ${service}."
  fi
}

validate_matching_orderbook_write_tokens() {
  local orderbook_file="$1"
  local api_file="$2"
  if [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" == "1" ]]; then
    return 0
  fi

  local orderbook_write_token
  local api_write_token
  orderbook_write_token="$(env_value POLYEDGE_ORDERBOOK__WRITE_TOKEN "${orderbook_file}")"
  api_write_token="$(env_value POLYEDGE_ORDERBOOK__WRITE_TOKEN "${api_file}")"
  if [[ "${orderbook_write_token}" != "${api_write_token}" ]]; then
    fail "POLYEDGE_ORDERBOOK__WRITE_TOKEN must match between ${orderbook_file} and ${api_file}."
  fi
}

ensure_env_file() {
  local file="$1"
  local example="$2"
  local label="$3"
  local -n created_ref="$4"

  if [[ -f "${file}" ]]; then
    return 0
  fi
  [[ -f "${example}" ]] || fail "${label} env example not found: ${example}"
  cp "${example}" "${file}"
  created_ref+=("${file}")
}

export_env_if_set() {
  local file="$1"
  local key="$2"
  local value
  value="$(env_value "${key}" "${file}")"
  if [[ -n "${value}" ]]; then
    export "${key}=${value}"
  fi
}

load_compose_interpolation_env() {
  if [[ -f "${api_env_file}" ]]; then
    export_env_if_set "${api_env_file}" POLYEDGE_API_IMAGE
    export_env_if_set "${api_env_file}" POLYEDGE_API_BIND
    export_env_if_set "${api_env_file}" POLYEDGE_API_PORT
  fi
  if [[ -f "${orderbook_env_file}" ]]; then
    export_env_if_set "${orderbook_env_file}" POLYEDGE_ORDERBOOK_IMAGE
    export_env_if_set "${orderbook_env_file}" POLYEDGE_ORDERBOOK_BIND
    export_env_if_set "${orderbook_env_file}" POLYEDGE_ORDERBOOK_PORT
  fi
  if [[ -f "${front_env_file}" ]]; then
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_IMAGE
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_BIND
    export_env_if_set "${front_env_file}" POLYEDGE_FRONT_PORT
  fi
}

# Compute a checksum of a file, or "MISSING" if it does not exist.
file_hash() {
  if [[ -f "$1" ]]; then
    md5sum "$1" 2>/dev/null | awk '{print $1}'
  else
    printf 'MISSING'
  fi
}

# Compute a checksum of frontend source files (excluding node_modules, .next, out).
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
  local source_hash
  local env_hash
  source_hash="$(frontend_hash "$1")"
  env_hash="$(file_hash "$2")"
  printf '%s\n%s\n' "${source_hash}" "${env_hash}" | md5sum | awk '{print $1}'
}

load_frontend_build_env() {
  local file="$1"
  [[ -f "${file}" ]] || fail "frontend env file not found: ${file}"

  local api_base_url
  api_base_url="$(env_value NEXT_PUBLIC_POLYEDGE_API_BASE_URL "${file}")"
  if [[ -z "${api_base_url}" ]]; then
    fail "NEXT_PUBLIC_POLYEDGE_API_BASE_URL must be set in ${file} before building the static frontend."
  fi
  export NEXT_PUBLIC_POLYEDGE_API_BASE_URL="${api_base_url}"

  local dev_bypass
  dev_bypass="$(env_value NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS "${file}")"
  if [[ -n "${dev_bypass}" ]]; then
    export NEXT_PUBLIC_POLYEDGE_INTERNAL_AUTH_DEV_BYPASS="${dev_bypass}"
  fi

  local console_auth
  console_auth="$(env_value NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH "${file}")"
  if [[ -n "${console_auth}" ]]; then
    export NEXT_PUBLIC_POLYEDGE_CONSOLE_AUTH="${console_auth}"
  fi

  log "frontend build env loaded from ${file}: NEXT_PUBLIC_POLYEDGE_API_BASE_URL=${NEXT_PUBLIC_POLYEDGE_API_BASE_URL}"
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
if (!outDir || !version) {
  process.exit(1);
}

function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, files);
    } else if (entry.isFile() && entry.name.endsWith(".html")) {
      files.push(fullPath);
    }
  }
  return files;
}

const pattern = /(\/_next\/static\/[^"'<>\\\s]+?\.(?:js|css))(?:\?v=[0-9a-f]+)?/g;
for (const file of walk(outDir)) {
  const current = fs.readFileSync(file, "utf8");
  const next = current.replace(pattern, `$1?v=${version}`);
  if (next !== current) {
    fs.writeFileSync(file, next);
  }
}
NODE
}

# Build frontend static files locally (yarn build -> out/).
build_frontend() {
  local front_dir="${deploy_dir}/packages/front"
  local asset_version
  [[ -f "${front_dir}/package.json" ]] || fail "packages/front/package.json not found"
  load_frontend_build_env "${front_env_file}"
  asset_version="$(frontend_build_hash "${front_dir}" "${front_env_file}")"
  log "building frontend static files (yarn build)"
  (cd "${front_dir}" && rm -rf .next out && yarn install --frozen-lockfile && yarn build) || fail "frontend yarn build failed"
  [[ -d "${front_dir}/out" ]] || fail "frontend build did not produce out/ directory"
  version_frontend_static_assets "${front_dir}" "${asset_version}"
  log "frontend build complete ($(du -sh "${front_dir}/out" | cut -f1))"
}

# Check if a docker compose service container is running.
container_running() {
  local service="$1"
  local status
  status="$(${compose_cmd} ps --format json "${service}" 2>/dev/null)" || true
  if [[ -z "${status}" ]]; then
    return 1
  fi
  printf '%s' "${status}" | tail -1 | grep -q '"running"' && return 0
  return 1
}

save_deploy_state() {
  local state_file="$1"
  cat > "${state_file}" <<EOF
api_hash=${current_api_hash}
orderbook_hash=${current_orderbook_hash}
front_hash=${current_front_hash}
commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
deployed_at=$(date '+%Y-%m-%d %H:%M:%S')
EOF
  log "deploy state saved to ${state_file}"
}

# Return 0 if the given service should be skipped based on POLYEDGE_SKIP_SERVICES.
should_skip_service() {
  local service="$1"
  local skip="${POLYEDGE_SKIP_SERVICES:-}"
  [[ -z "${skip}" ]] && return 1
  local IFS=','
  for s in ${skip}; do
    s="${s,,}"
    s="${s#polyedge-}"
    if [[ "${s}" == "${service}" ]]; then
      return 0
    fi
  done
  return 1
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_root="$(cd "${script_dir}/.." && pwd)"
deploy_dir="${POLYEDGE_DEPLOY_DIR:-${default_root}}"
repo_url="${POLYEDGE_GIT_REPO:-}"
branch="${POLYEDGE_GIT_BRANCH:-}"
skip_git_pull="${POLYEDGE_SKIP_GIT_PULL:-0}"

target_api=0
target_front=0
target_orderbook=0

parse_targets target_api target_front target_orderbook "$@"

# ---- setup logging: tee to file when running non-interactively -----------
log_file="${POLYEDGE_LOG_FILE:-}"
if [[ -z "${log_file}" && ! -t 0 ]]; then
  log_file="${HOME:-/tmp}/polyedge-deploy.log"
fi
if [[ -n "${log_file}" ]]; then
  if mkdir -p "$(dirname "${log_file}")" && touch "${log_file}" 2>/dev/null; then
    exec > >(tee -a "${log_file}") 2>&1
  else
    printf '[polyedge-deploy] %s WARN: cannot write log file %s; continuing with stdout/stderr only\n' "$(date '+%Y-%m-%d %H:%M:%S')" "${log_file}" >&2
  fi
fi

lock_file="${POLYEDGE_DEPLOY_LOCK_FILE:-/tmp/polyedge-deploy.lock}"
if command -v flock >/dev/null 2>&1; then
  exec 9>"${lock_file}"
  if ! flock -n 9; then
    log "another deploy is running; skip (${lock_file})"
    exit 0
  fi
else
  log "flock is not installed; continuing without deploy lock"
fi

log "=== deploy start (mode=${mode}) ==="

# ---- git clone (first-time) ---------------------------------------------
if [[ ! -d "${deploy_dir}/.git" ]]; then
  [[ -n "${repo_url}" ]] || fail "POLYEDGE_DEPLOY_DIR is not a git checkout. Set POLYEDGE_GIT_REPO to clone from GitHub."
  [[ -n "${branch}" ]] || branch="main"

  if [[ -e "${deploy_dir}" ]] && [[ -n "$(find "${deploy_dir}" -mindepth 1 -maxdepth 1 2>/dev/null)" ]]; then
    fail "POLYEDGE_DEPLOY_DIR exists and is not empty: ${deploy_dir}"
  fi

  mkdir -p "$(dirname "${deploy_dir}")"
  log "cloning ${repo_url} branch ${branch} into ${deploy_dir}"
  git clone --branch "${branch}" "${repo_url}" "${deploy_dir}"
fi

cd "${deploy_dir}"

if [[ -z "${branch}" ]]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  if [[ "${branch}" == "HEAD" ]]; then
    branch="main"
  fi
fi

# ---- git pull ------------------------------------------------------------
new_code=0
pre_merge_head=""

if [[ "${skip_git_pull}" != "1" ]]; then
  if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    fail "tracked files have local changes. Commit, stash, or set POLYEDGE_SKIP_GIT_PULL=1."
  fi

  log "fetching latest code from origin/${branch}"
  git fetch --prune origin "${branch}"

  current_branch="$(git rev-parse --abbrev-ref HEAD)"
  if [[ "${current_branch}" != "${branch}" ]]; then
    log "checking out ${branch}"
    if git show-ref --verify --quiet "refs/heads/${branch}"; then
      git checkout "${branch}"
    else
      git checkout -B "${branch}" "origin/${branch}"
    fi
  fi

  local_head="$(git rev-parse HEAD)"
  remote_head="$(git rev-parse "origin/${branch}")"

  if [[ "${local_head}" != "${remote_head}" ]]; then
    pre_merge_head="${local_head}"
    log "fast-forwarding ${branch} (${local_head:0:8} -> ${remote_head:0:8})"
    git merge --ff-only "origin/${branch}"
    new_code=1
  else
    log "already up-to-date (${local_head:0:8})"
  fi
else
  log "skipping git update"
fi

# ---- compose & env setup -------------------------------------------------
compose_file="${POLYEDGE_COMPOSE_FILE:-${deploy_dir}/deploy/docker-compose.yml}"
deploy_dir_path="${deploy_dir}/deploy"
api_env_file="${POLYEDGE_API_ENV_FILE:-${deploy_dir_path}/.env.api}"
orderbook_env_file="${POLYEDGE_ORDERBOOK_ENV_FILE:-${deploy_dir_path}/.env.orderbook}"
front_env_file="${POLYEDGE_FRONT_ENV_FILE:-${deploy_dir_path}/.env.front}"

[[ -f "${compose_file}" ]] || fail "compose file not found: ${compose_file}"

created_env_files=()
if [[ "${mode}" == "auto" ]]; then
  if ! should_skip_service api; then
    ensure_env_file "${api_env_file}" "${deploy_dir_path}/.env.api.example" "api" created_env_files
  fi
  if ! should_skip_service orderbook; then
    ensure_env_file "${orderbook_env_file}" "${deploy_dir_path}/.env.orderbook.example" "orderbook" created_env_files
  fi
  if ! should_skip_service front; then
    ensure_env_file "${front_env_file}" "${deploy_dir_path}/.env.front.example" "front" created_env_files
  fi
else
  if [[ "${target_api}" == "1" ]]; then
    ensure_env_file "${api_env_file}" "${deploy_dir_path}/.env.api.example" "api" created_env_files
  fi
  if [[ "${target_orderbook}" == "1" ]]; then
    ensure_env_file "${orderbook_env_file}" "${deploy_dir_path}/.env.orderbook.example" "orderbook" created_env_files
  fi
  if [[ "${target_front}" == "1" ]]; then
    ensure_env_file "${front_env_file}" "${deploy_dir_path}/.env.front.example" "front" created_env_files
  fi
fi

if [[ ${#created_env_files[@]} -gt 0 ]]; then
  fail "created env file(s): ${created_env_files[*]}. Edit PostgreSQL URLs, matching orderbook write tokens, frontend API URL, and auth settings, then rerun this script."
fi

if [[ "${mode}" == "auto" ]]; then
  if ! should_skip_service orderbook; then
    validate_postgres_env_file "${orderbook_env_file}" "orderbook"
    validate_orderbook_write_token "${orderbook_env_file}" "orderbook"
  fi
  if ! should_skip_service api; then
    validate_api_env_file "${api_env_file}"
  fi
  if ! should_skip_service orderbook && ! should_skip_service api; then
    validate_matching_orderbook_write_tokens \
      "${orderbook_env_file}" \
      "${api_env_file}"
  fi
else
  if [[ "${target_orderbook}" == "1" ]]; then
    validate_postgres_env_file "${orderbook_env_file}" "orderbook"
    validate_orderbook_write_token "${orderbook_env_file}" "orderbook"
  fi
  if [[ "${target_api}" == "1" ]]; then
    validate_api_env_file "${api_env_file}"
  fi
  if [[ "${target_orderbook}" == "1" && "${target_api}" == "1" ]]; then
    validate_matching_orderbook_write_tokens \
      "${orderbook_env_file}" \
      "${api_env_file}"
  fi
fi

compose_cmd="$(find_compose)" || fail "Docker Compose is not installed."
export COMPOSE_PARALLEL_LIMIT="${COMPOSE_PARALLEL_LIMIT:-1}"
load_compose_interpolation_env

# ---------------------------------------------------------------------------
# Auto mode: per-service intelligent change detection
# ---------------------------------------------------------------------------
if [[ "${mode}" == "auto" ]]; then
  current_api_hash="$(file_hash bin/polyedge-api)"
  current_orderbook_hash="$(file_hash bin/polyedge-orderbook)"
  current_front_hash="$(frontend_build_hash packages/front "${front_env_file}")"

  state_file="${deploy_dir}/.deploy-state"
  saved_api_hash=""
  saved_orderbook_hash=""
  saved_front_hash=""

  if [[ -f "${state_file}" ]]; then
    saved_api_hash="$(grep '^api_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_orderbook_hash="$(grep '^orderbook_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_front_hash="$(grep '^front_hash=' "${state_file}" | cut -d= -f2 || true)"
  fi

  # Per-service change detection
  api_image_changed=0
  orderbook_changed=0
  front_changed=0

  if [[ "${current_api_hash}" != "${saved_api_hash}" ]]; then
    api_image_changed=1
    log "api binary changed (${saved_api_hash:-NONE}->${current_api_hash:0:8})"
  fi

  if [[ "${current_orderbook_hash}" != "${saved_orderbook_hash}" ]]; then
    orderbook_changed=1
    log "orderbook binary changed (${saved_orderbook_hash:-NONE}->${current_orderbook_hash:0:8})"
  fi

  if [[ "${new_code}" == "1" && -n "${pre_merge_head}" ]]; then
    if git diff --name-only "${pre_merge_head}" HEAD -- packages/front/ packages/front/Dockerfile packages/front/nginx.conf.template | grep -q .; then
      front_changed=1
      log "frontend files changed in new commits"
    fi
  fi
  if [[ "${front_changed}" == "0" && "${current_front_hash}" != "${saved_front_hash}" ]]; then
    front_changed=1
    log "frontend files changed on disk (${saved_front_hash:-NONE}->${current_front_hash:0:8})"
  fi

  # Per-service container status
  api_running=1
  orderbook_running=1
  front_running=1

  if ! should_skip_service api && ! container_running polyedge-api; then
    log "polyedge-api container is not running"
    api_running=0
  fi
  if ! should_skip_service orderbook && ! container_running polyedge-orderbook; then
    log "polyedge-orderbook container is not running"
    orderbook_running=0
  fi
  if ! should_skip_service front && ! container_running polyedge-front; then
    log "polyedge-front container is not running"
    front_running=0
  fi

  # Decide which images to build (per image, not per service)
  build_images=()
  restart_services=()

  # api image
  if [[ "${api_image_changed}" == "1" ]]; then
    build_images+=(polyedge-api)
  fi
  if [[ "${api_image_changed}" == "1" || "${api_running}" == "0" ]]; then
    if ! should_skip_service api; then
      [[ -f bin/polyedge-api ]] || fail "bin/polyedge-api is missing. Build it with scripts/build-backend-bin.sh."
      restart_services+=(polyedge-api)
    fi
  fi

  # orderbook image
  if [[ "${orderbook_changed}" == "1" ]]; then
    build_images+=(polyedge-orderbook)
  fi
  if [[ "${orderbook_changed}" == "1" || "${orderbook_running}" == "0" ]]; then
    if ! should_skip_service orderbook; then
      [[ -f bin/polyedge-orderbook ]] || fail "bin/polyedge-orderbook is missing. Build it with scripts/build-backend-bin.sh."
      restart_services+=(polyedge-orderbook)
    fi
  fi

  # frontend image
  if [[ "${front_changed}" == "1" ]]; then
    build_images+=(polyedge-front)
  fi
  if [[ "${front_changed}" == "1" || "${front_running}" == "0" ]]; then
    if ! should_skip_service front; then
      restart_services+=(polyedge-front)
    fi
  fi

  # Nothing to do?
  if [[ ${#build_images[@]} -eq 0 && ${#restart_services[@]} -eq 0 ]]; then
    log "no changes detected and all targeted containers running -> nothing to do"
    log "=== deploy end (skipped) ==="
    exit 0
  fi

  if [[ ${#build_images[@]} -gt 0 ]]; then
    if printf '%s\n' "${build_images[@]}" | grep -qx 'polyedge-front'; then
      build_frontend
    fi
    log "building images: ${build_images[*]} (COMPOSE_PARALLEL_LIMIT=${COMPOSE_PARALLEL_LIMIT})"
    ${compose_cmd} -f "${compose_file}" build --pull "${build_images[@]}"
    save_deploy_state "${state_file}"
  else
    log "no image changes detected; starting existing images"
  fi

  if [[ ${#restart_services[@]} -gt 0 ]]; then
    log "starting containers: ${restart_services[*]}"
    ${compose_cmd} -f "${compose_file}" up -d --remove-orphans "${restart_services[@]}"
  fi

else
  # ---------------------------------------------------------------------------
  # Manual mode: explicit targets, per-service binary checks
  # ---------------------------------------------------------------------------
  build_images=()
  runtime_services=()

  # Determine which images need building (only if targeted services need them)
  if [[ "${target_api}" == "1" ]]; then
    build_images+=(polyedge-api)
  fi
  if [[ "${target_orderbook}" == "1" ]]; then
    build_images+=(polyedge-orderbook)
  fi
  if [[ "${target_front}" == "1" ]]; then
    build_images+=(polyedge-front)
  fi

  # Only check binaries for targeted services
  if [[ "${target_api}" == "1" ]]; then
    [[ -f "bin/polyedge-api" ]] || fail "bin/polyedge-api is missing. Build it with: POLYEDGE_BACKEND_BINARY=polyedge-api scripts/build-backend-bin.sh"
  fi
  if [[ "${target_orderbook}" == "1" ]]; then
    [[ -f "bin/polyedge-orderbook" ]] || fail "bin/polyedge-orderbook is missing. Build it with: POLYEDGE_BACKEND_BINARY=polyedge-orderbook scripts/build-backend-bin.sh"
  fi

  # Collect runtime services
  if [[ "${target_orderbook}" == "1" ]]; then
    runtime_services+=(polyedge-orderbook)
  fi
  if [[ "${target_api}" == "1" ]]; then
    runtime_services+=(polyedge-api)
  fi
  if [[ "${target_front}" == "1" ]]; then
    runtime_services+=(polyedge-front)
  fi

  if printf '%s\n' "${build_images[@]}" | grep -qx 'polyedge-front' 2>/dev/null; then
    build_frontend
  fi

  if [[ ${#build_images[@]} -gt 0 ]]; then
    log "building images: ${build_images[*]} (COMPOSE_PARALLEL_LIMIT=${COMPOSE_PARALLEL_LIMIT})"
    ${compose_cmd} -f "${compose_file}" build --pull "${build_images[@]}"
  fi

  if [[ ${#runtime_services[@]} -gt 0 ]]; then
    log "starting containers: ${runtime_services[*]}"
    ${compose_cmd} -f "${compose_file}" up -d --remove-orphans "${runtime_services[@]}"
  fi
fi

log "current container status"
${compose_cmd} -f "${compose_file}" ps

log "=== deploy end ==="
