#!/usr/bin/env bash
set -Eeuo pipefail

# ---------------------------------------------------------------------------
# PolyEdge deploy script
#
# Usage:
#   scripts/deploy.sh                 # auto mode (default for cron/CI)
#   scripts/deploy.sh auto            # same as no args
#   scripts/deploy.sh all             # force rebuild everything
#   scripts/deploy.sh api [orderbook|worker|front ...]
#
# Auto mode (default):
#   1. git fetch + fast-forward
#   2. If no changes detected AND all containers are running -> skip entirely
#   3. If a backend binary changed -> rebuild backend image -> restart orderbook, api & worker
#   4. If frontend files changed (packages/front/) -> rebuild frontend image -> restart front
#   5. If any container is not running -> start existing image without forcing a rebuild
#   6. Persist image build state to .deploy-state immediately after successful builds
#
# Cron example (every 5 minutes; deploy.sh also has an internal lock):
#   */5 * * * * POLYEDGE_LOG_FILE=/home/polyedge/polyedge-deploy.log /path/to/scripts/deploy.sh
#
# Environment variables:
#   POLYEDGE_DEPLOY_DIR       - repo checkout (default: script's parent)
#   POLYEDGE_GIT_REPO         - remote URL (only used for first clone)
#   POLYEDGE_GIT_BRANCH       - branch to track (default: current)
#   POLYEDGE_COMPOSE_FILE     - docker-compose file path
#   POLYEDGE_ENV_FILE         - .env file path
#   POLYEDGE_SKIP_ENV_VALIDATION=1 - skip .env sanity checks
#   POLYEDGE_LOG_FILE         - log file path (default for cron: $HOME/polyedge-deploy.log)
#   POLYEDGE_DEPLOY_LOCK_FILE - non-overlap lock file (default: /tmp/polyedge-deploy.lock)
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
  all             Force rebuild backend and frontend images, then restart all services.
  orderbook       Rebuild the backend image and restart only the orderbook service.
  api worker      Rebuild the backend image and restart selected backend services.
  api             Rebuild the backend image and restart only the API service.
  worker          Rebuild the backend image and restart only the worker service.
  front           Rebuild the frontend image and restart only the frontend service.

Multiple targets can be passed as separate args or comma-separated, for example:
  scripts/deploy.sh api worker
  scripts/deploy.sh api,front
EOF
}

# mode: "auto" (change-detection) or "manual" (explicit targets)
mode="auto"

parse_targets() {
  local -n target_api_ref="$1"
  local -n target_worker_ref="$2"
  local -n target_front_ref="$3"
  local -n target_orderbook_ref="$4"
  local raw
  local target
  local part
  local -a parts

  shift 4

  if [[ $# -eq 0 ]]; then
    # default: auto mode
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
          target_api_ref=1
          target_worker_ref=1
          target_front_ref=1
          target_orderbook_ref=1
          ;;
        api)
          mode="manual"
          target_api_ref=1
          ;;
        worker)
          mode="manual"
          target_worker_ref=1
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

validate_env_file() {
  local file="$1"

  if [[ "${POLYEDGE_SKIP_ENV_VALIDATION:-0}" == "1" ]]; then
    log "skipping env validation because POLYEDGE_SKIP_ENV_VALIDATION=1"
    return 0
  fi

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

  local step_up_code
  step_up_code="$(env_value POLYEDGE_AUTH__STEP_UP_CODE "${file}")"
  if [[ -z "${step_up_code}" || "${step_up_code}" == "change-me" ]]; then
    fail "POLYEDGE_AUTH__STEP_UP_CODE must be set to a non-placeholder value in ${file}."
  fi

  local runtime_environment
  runtime_environment="$(env_value POLYEDGE_RUNTIME__ENVIRONMENT "${file}")"
  runtime_environment="${runtime_environment:-local}"
  local dev_bypass
  dev_bypass="$(env_value POLYEDGE_INTERNAL_AUTH_DEV_BYPASS "${file}")"
  dev_bypass="${dev_bypass:-1}"
  if [[ "${runtime_environment}" != "local" && "${dev_bypass}" == "1" ]]; then
    fail "POLYEDGE_INTERNAL_AUTH_DEV_BYPASS=1 is only allowed with POLYEDGE_RUNTIME__ENVIRONMENT=local."
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

# Build frontend static files locally (yarn build -> out/).
# The Dockerfile copies out/ into the nginx image without container-side compilation.
build_frontend() {
  local front_dir="${deploy_dir}/packages/front"
  [[ -f "${front_dir}/package.json" ]] || fail "packages/front/package.json not found"
  log "building frontend static files (yarn build)"
  (cd "${front_dir}" && yarn install --frozen-lockfile && yarn build) || fail "frontend yarn build failed"
  [[ -d "${front_dir}/out" ]] || fail "frontend build did not produce out/ directory"
  log "frontend build complete ($(du -sh "${front_dir}/out" | cut -f1))"
}

# Check if a docker compose service container is running.
# Returns 0 if running, 1 if not.
container_running() {
  local service="$1"
  local status
  status="$(${compose_cmd} ps --format json "${service}" 2>/dev/null)" || true
  if [[ -z "${status}" ]]; then
    return 1
  fi
  # docker compose ps --format json may return multiple lines; check last
  printf '%s' "${status}" | tail -1 | grep -q '"running"' && return 0
  return 1
}

save_deploy_state() {
  local state_file="$1"
  cat > "${state_file}" <<EOF
api_hash=${current_api_hash}
worker_hash=${current_worker_hash}
orderbook_hash=${current_orderbook_hash}
front_hash=${current_front_hash}
commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
deployed_at=$(date '+%Y-%m-%d %H:%M:%S')
EOF
  log "deploy state saved to ${state_file}"
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
target_worker=0
target_front=0
target_orderbook=0

parse_targets target_api target_worker target_front target_orderbook "$@"

# ---- setup logging: tee to file when running non-interactively -----------
log_file="${POLYEDGE_LOG_FILE:-}"
if [[ -z "${log_file}" && ! -t 0 ]]; then
  # non-interactive (cron) -> default to a user-writable log file
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

  # Detect if there are new commits to pull
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
env_file="${POLYEDGE_ENV_FILE:-${deploy_dir}/deploy/.env}"
env_example="${deploy_dir}/deploy/.env.example"
deploy_dir_path="${deploy_dir}/deploy"

[[ -f "${compose_file}" ]] || fail "compose file not found: ${compose_file}"

if [[ ! -f "${env_file}" ]]; then
  [[ -f "${env_example}" ]] || fail "env example not found: ${env_example}"
  cp "${env_example}" "${env_file}"
  # Copy service-specific env examples if they don't exist yet.
  for suffix in api orderbook worker front; do
    local_example="${deploy_dir_path}/.env.${suffix}.example"
    local_target="${deploy_dir_path}/.env.${suffix}"
    if [[ -f "${local_example}" && ! -f "${local_target}" ]]; then
      cp "${local_example}" "${local_target}"
    fi
  done
  fail "created ${env_file} and service env files. Edit the PostgreSQL URL and console step-up code, then rerun this script."
fi

validate_env_file "${env_file}"

compose_cmd="$(find_compose)" || fail "Docker Compose is not installed."
export COMPOSE_PARALLEL_LIMIT="${COMPOSE_PARALLEL_LIMIT:-1}"

# ---------------------------------------------------------------------------
# Auto mode: intelligent change detection via persistent state file
# ---------------------------------------------------------------------------
if [[ "${mode}" == "auto" ]]; then
  # --- Compute current state ------------------------------------------------
  current_api_hash="$(file_hash bin/polyedge-api)"
  current_worker_hash="$(file_hash bin/polyedge-worker)"
  current_orderbook_hash="$(file_hash bin/polyedge-orderbook)"
  current_front_hash="$(frontend_hash packages/front)"

  # --- Load last deployed state ---------------------------------------------
  state_file="${deploy_dir}/.deploy-state"
  saved_api_hash=""
  saved_worker_hash=""
  saved_orderbook_hash=""
  saved_front_hash=""

  if [[ -f "${state_file}" ]]; then
    saved_api_hash="$(grep '^api_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_worker_hash="$(grep '^worker_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_orderbook_hash="$(grep '^orderbook_hash=' "${state_file}" | cut -d= -f2 || true)"
    saved_front_hash="$(grep '^front_hash=' "${state_file}" | cut -d= -f2 || true)"
  fi

  backend_changed=0
  front_changed=0

  # --- Detect backend changes ------------------------------------------------
  if [[ "${current_api_hash}" != "${saved_api_hash}" || "${current_worker_hash}" != "${saved_worker_hash}" || "${current_orderbook_hash}" != "${saved_orderbook_hash}" ]]; then
    backend_changed=1
    log "backend binary changed (api: ${saved_api_hash:-NONE}->${current_api_hash:0:8}, worker: ${saved_worker_hash:-NONE}->${current_worker_hash:0:8}, orderbook: ${saved_orderbook_hash:-NONE}->${current_orderbook_hash:0:8})"
  fi

  # --- Detect frontend changes -----------------------------------------------
  if [[ "${new_code}" == "1" && -n "${pre_merge_head}" ]]; then
    # Check if any frontend-related files changed in the pulled commits
    if git diff --name-only "${pre_merge_head}" HEAD -- packages/front/ packages/front/Dockerfile packages/front/nginx.conf.template | grep -q .; then
      front_changed=1
      log "frontend files changed in new commits"
    fi
  fi
  if [[ "${front_changed}" == "0" && "${current_front_hash}" != "${saved_front_hash}" ]]; then
    front_changed=1
    log "frontend files changed on disk (${saved_front_hash:-NONE}->${current_front_hash:0:8})"
  fi

  # --- Check container status: down services are started without forcing rebuild
  backend_running=1
  front_running=1

  if ! container_running polyedge-api; then
    log "polyedge-api container is not running"
    backend_running=0
  fi
  if ! container_running polyedge-orderbook; then
    log "polyedge-orderbook container is not running"
    backend_running=0
  fi
  if ! container_running polyedge-worker; then
    log "polyedge-worker container is not running"
    backend_running=0
  fi
  if ! container_running polyedge-front; then
    log "polyedge-front container is not running"
    front_running=0
  fi

  # --- Decide what to build and restart --------------------------------------
  build_services=()
  restart_services=()

  if [[ "${backend_changed}" == "1" ]]; then
    build_services+=(polyedge-api)
  fi
  if [[ "${backend_changed}" == "1" || "${backend_running}" == "0" ]]; then
    restart_services+=(polyedge-orderbook polyedge-api polyedge-worker)
  fi

  if [[ "${front_changed}" == "1" ]]; then
    build_services+=(polyedge-front)
  fi
  if [[ "${front_changed}" == "1" || "${front_running}" == "0" ]]; then
    restart_services+=(polyedge-front)
  fi

  # --- Nothing to do? -------------------------------------------------------
  if [[ ${#build_services[@]} -eq 0 && ${#restart_services[@]} -eq 0 ]]; then
    log "no changes detected and all containers running -> nothing to do"
    log "=== deploy end (skipped) ==="
    exit 0
  fi

  # --- Ensure binaries exist before building backend -------------------------
  if [[ "${backend_changed}" == "1" ]]; then
    [[ -f bin/polyedge-api ]] || fail "bin/polyedge-api is missing. Build it with scripts/build-backend-bin.sh."
    [[ -f bin/polyedge-worker ]] || fail "bin/polyedge-worker is missing. Build it with scripts/build-backend-bin.sh."
    [[ -f bin/polyedge-orderbook ]] || fail "bin/polyedge-orderbook is missing. Build it with scripts/build-backend-bin.sh."
  fi

  if [[ ${#build_services[@]} -gt 0 ]]; then
    # Build frontend static files locally before Docker image build
    if printf '%s\n' "${build_services[@]}" | grep -qx 'polyedge-front'; then
      build_frontend
    fi
    log "building images: ${build_services[*]} (COMPOSE_PARALLEL_LIMIT=${COMPOSE_PARALLEL_LIMIT})"
    ${compose_cmd} --env-file "${env_file}" -f "${compose_file}" build --pull "${build_services[@]}"
    save_deploy_state "${state_file}"
  else
    log "no image changes detected; starting existing images"
  fi

  log "starting containers: ${restart_services[*]}"
  ${compose_cmd} --env-file "${env_file}" -f "${compose_file}" up -d --remove-orphans "${restart_services[@]}"

else
  # ---------------------------------------------------------------------------
  # Manual mode: explicit targets (same as original behavior)
  # ---------------------------------------------------------------------------
  build_services=()
  runtime_services=()

  if [[ "${target_api}" == "1" || "${target_worker}" == "1" || "${target_orderbook}" == "1" ]]; then
    build_services+=(polyedge-api)
  fi
  if [[ "${target_front}" == "1" ]]; then
    build_services+=(polyedge-front)
  fi

  if [[ "${target_orderbook}" == "1" ]]; then
    runtime_services+=(polyedge-orderbook)
  fi
  if [[ "${target_api}" == "1" ]]; then
    runtime_services+=(polyedge-api)
  fi
  if [[ "${target_worker}" == "1" ]]; then
    runtime_services+=(polyedge-worker)
  fi
  if [[ "${target_front}" == "1" ]]; then
    runtime_services+=(polyedge-front)
  fi

  if [[ "${target_api}" == "1" || "${target_worker}" == "1" || "${target_orderbook}" == "1" ]]; then
    [[ -f "bin/polyedge-api" ]] || fail "bin/polyedge-api is missing. Build it with scripts/build-backend-bin.sh and commit it."
    [[ -f "bin/polyedge-worker" ]] || fail "bin/polyedge-worker is missing. Build it with scripts/build-backend-bin.sh and commit it."
    [[ -f "bin/polyedge-orderbook" ]] || fail "bin/polyedge-orderbook is missing. Build it with scripts/build-backend-bin.sh and commit it."
  fi

  # Build frontend static files locally before Docker image build
  if printf '%s\n' "${build_services[@]}" | grep -qx 'polyedge-front'; then
    build_frontend
  fi

  log "building images: ${build_services[*]} (COMPOSE_PARALLEL_LIMIT=${COMPOSE_PARALLEL_LIMIT})"
  ${compose_cmd} --env-file "${env_file}" -f "${compose_file}" build --pull "${build_services[@]}"

  log "starting containers: ${runtime_services[*]}"
  ${compose_cmd} --env-file "${env_file}" -f "${compose_file}" up -d --remove-orphans "${runtime_services[@]}"
fi

log "current container status"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" ps

log "=== deploy end ==="
