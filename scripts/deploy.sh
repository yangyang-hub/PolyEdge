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

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_root="$(cd "${script_dir}/.." && pwd)"
deploy_dir="${POLYEDGE_DEPLOY_DIR:-${default_root}}"
repo_url="${POLYEDGE_GIT_REPO:-}"
branch="${POLYEDGE_GIT_BRANCH:-}"
skip_git_pull="${POLYEDGE_SKIP_GIT_PULL:-0}"

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

compose_file="${POLYEDGE_COMPOSE_FILE:-${deploy_dir}/deploy/docker-compose.yml}"
env_file="${POLYEDGE_ENV_FILE:-${deploy_dir}/deploy/.env}"
env_example="${deploy_dir}/deploy/.env.example"

[[ -f "${compose_file}" ]] || fail "compose file not found: ${compose_file}"

if [[ ! -f "${env_file}" ]]; then
  [[ -f "${env_example}" ]] || fail "env example not found: ${env_example}"
  cp "${env_example}" "${env_file}"
  fail "created ${env_file}. Edit PostgreSQL/Redis URLs and secrets, then rerun this script."
fi

compose_cmd="$(find_compose)" || fail "Docker Compose is not installed."

log "building images"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" build --pull

log "starting containers"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" up -d --remove-orphans

log "current container status"
${compose_cmd} --env-file "${env_file}" -f "${compose_file}" ps
