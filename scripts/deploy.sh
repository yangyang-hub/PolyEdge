#!/usr/bin/env bash
set -Eeuo pipefail

log() {
  printf '[polyedge-deploy] %s\n' "$*"
}

fail() {
  printf '[polyedge-deploy] ERROR: %s\n' "$*" >&2
  exit 1
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
  step_up_code="$(env_value POLYEDGE_CONSOLE_STEP_UP_CODE "${file}")"
  if [[ -z "${step_up_code}" || "${step_up_code}" == "change-me" ]]; then
    fail "POLYEDGE_CONSOLE_STEP_UP_CODE must be set to a non-placeholder value in ${file}."
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

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_root="$(cd "${script_dir}/.." && pwd)"
deploy_dir="${POLYEDGE_DEPLOY_DIR:-${default_root}}"
repo_url="${POLYEDGE_GIT_REPO:-}"
branch="${POLYEDGE_GIT_BRANCH:-}"
skip_git_pull="${POLYEDGE_SKIP_GIT_PULL:-0}"
force_rebuild="${POLYEDGE_FORCE_REBUILD:-0}"
old_rev=""
did_clone=0

if [[ ! -d "${deploy_dir}/.git" ]]; then
  [[ -n "${repo_url}" ]] || fail "POLYEDGE_DEPLOY_DIR is not a git checkout. Set POLYEDGE_GIT_REPO to clone from GitHub."
  [[ -n "${branch}" ]] || branch="main"

  if [[ -e "${deploy_dir}" ]] && [[ -n "$(find "${deploy_dir}" -mindepth 1 -maxdepth 1 2>/dev/null)" ]]; then
    fail "POLYEDGE_DEPLOY_DIR exists and is not empty: ${deploy_dir}"
  fi

  mkdir -p "$(dirname "${deploy_dir}")"
  log "cloning ${repo_url} branch ${branch} into ${deploy_dir}"
  git clone --branch "${branch}" "${repo_url}" "${deploy_dir}"
  did_clone=1
fi

cd "${deploy_dir}"
old_rev="$(git rev-parse HEAD)"

if [[ -z "${branch}" ]]; then
  branch="$(git rev-parse --abbrev-ref HEAD)"
  if [[ "${branch}" == "HEAD" ]]; then
    branch="main"
  fi
fi

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

  log "fast-forwarding ${branch}"
  git merge --ff-only "origin/${branch}"
else
  log "skipping git update"
fi

new_rev="$(git rev-parse HEAD)"

compose_file="${POLYEDGE_COMPOSE_FILE:-${deploy_dir}/deploy/docker-compose.yml}"
env_file="${POLYEDGE_ENV_FILE:-${deploy_dir}/deploy/.env}"
env_example="${deploy_dir}/deploy/.env.example"

[[ -f "${compose_file}" ]] || fail "compose file not found: ${compose_file}"

if [[ ! -f "${env_file}" ]]; then
  [[ -f "${env_example}" ]] || fail "env example not found: ${env_example}"
  cp "${env_example}" "${env_file}"
  fail "created ${env_file}. Edit PostgreSQL/Redis URLs and secrets, then rerun this script."
fi

validate_env_file "${env_file}"

compose_cmd="$(find_compose)" || fail "Docker Compose is not installed."

rebuild_services=()

if [[ "${did_clone}" == "1" ]]; then
  log "initial checkout detected"
  rebuild_services=(polyedge-api polyedge-front)
elif [[ "${force_rebuild}" == "1" ]]; then
  log "force rebuild requested"
  rebuild_services=(polyedge-api polyedge-front)
elif [[ "${old_rev}" != "${new_rev}" ]]; then
  log "checking changed deployment inputs from ${old_rev} to ${new_rev}"

  if ! git diff --quiet "${old_rev}" "${new_rev}" -- \
    bin/polyedge-api \
    bin/polyedge-worker \
    packages/backend/Dockerfile \
    deploy/docker-compose.yml \
    .dockerignore
  then
    rebuild_services+=(polyedge-api)
  fi

  if ! git diff --quiet "${old_rev}" "${new_rev}" -- \
    packages/front \
    deploy/docker-compose.yml \
    .dockerignore
  then
    rebuild_services+=(polyedge-front)
  fi
else
  log "repository is already up to date"
fi

if [[ ${#rebuild_services[@]} -gt 0 ]]; then
  if printf '%s\n' "${rebuild_services[@]}" | grep -qx 'polyedge-api'; then
    [[ -f "${deploy_dir}/bin/polyedge-api" ]] || fail "bin/polyedge-api is missing. Build it with scripts/build-backend-bin.sh and commit it."
    [[ -f "${deploy_dir}/bin/polyedge-worker" ]] || fail "bin/polyedge-worker is missing. Build it with scripts/build-backend-bin.sh and commit it."
  fi

  log "building changed images: ${rebuild_services[*]}"
  ${compose_cmd} --env-file "${env_file}" -f "${compose_file}" build --pull "${rebuild_services[@]}"
else
  log "no image rebuild needed"
fi

log "starting containers"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" up -d --remove-orphans

log "current container status"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" ps
