#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="${ROOT_DIR}/packages"
API_BASE_URL="${POLYEDGE_API_BASE_URL:-http://127.0.0.1:38001}"
FRONT_BASE_URL="${POLYEDGE_FRONT_BASE_URL:-}"
RUN_WORKER="${POLYEDGE_SMOKE_RUN_WORKER:-1}"
REQUEST_ID="${POLYEDGE_SMOKE_REQUEST_ID:-req_smoke_arbitrage_radar}"
BEARER_TOKEN="${POLYEDGE_SMOKE_BEARER_TOKEN:-}"

curl_headers=(
  -H "X-Request-Id: ${REQUEST_ID}"
)

if [[ -n "${BEARER_TOKEN}" ]]; then
  curl_headers+=(-H "Authorization: Bearer ${BEARER_TOKEN}")
else
  curl_headers+=(
    -H "X-PolyEdge-Dev-Auth: local"
    -H "X-PolyEdge-Console-Role: viewer"
    -H "X-PolyEdge-Console-User: Local%20Smoke"
  )
fi

check_json_endpoint() {
  local path="$1"
  local label="$2"
  local output_file
  output_file="$(mktemp)"

  local status
  status="$(curl -sS -o "${output_file}" -w "%{http_code}" "${curl_headers[@]}" "${API_BASE_URL}${path}")"
  if [[ -z "${BEARER_TOKEN}" && ( "${status}" == "401" || "${status}" == "403" ) ]]; then
    echo "SKIP ${label}: protected endpoint returned HTTP ${status}; set POLYEDGE_SMOKE_BEARER_TOKEN to require this check"
    rm -f "${output_file}"
    return 0
  fi

  if [[ "${status}" != "200" ]]; then
    echo "FAIL ${label}: HTTP ${status}" >&2
    cat "${output_file}" >&2
    rm -f "${output_file}"
    exit 1
  fi

  rm -f "${output_file}"
  echo "PASS ${label}"
}

echo "Checking PolyEdge API at ${API_BASE_URL}"
curl -fsS "${API_BASE_URL}/healthz" >/dev/null
echo "PASS api healthz"
curl -fsS "${API_BASE_URL}/readyz" >/dev/null
echo "PASS api readyz"

if [[ "${RUN_WORKER}" == "1" ]]; then
  echo "Running one arbitrage scan and analysis via worker"
  (
    cd "${BACKEND_DIR}"
    cargo run -p polyedge-worker -- scan-arbitrage-once
    cargo run -p polyedge-worker -- analyze-arbitrage-opportunities
  )
fi

check_json_endpoint "/api/v1/arbitrage/scans?limit=1" "arbitrage scans"
check_json_endpoint "/api/v1/arbitrage/opportunities?limit=5" "arbitrage opportunities"
check_json_endpoint "/api/v1/arbitrage/analysis?limit=1" "arbitrage analysis"

if [[ -n "${FRONT_BASE_URL}" ]]; then
  echo "Checking PolyEdge front at ${FRONT_BASE_URL}"
  curl -fsS "${FRONT_BASE_URL}/healthz" >/dev/null
  echo "PASS front healthz"
  curl -fsS -I "${FRONT_BASE_URL}/radar" >/dev/null
  echo "PASS front radar"
fi

echo "Arbitrage radar smoke completed"
