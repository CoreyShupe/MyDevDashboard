#!/usr/bin/env bash
# Isolated SANDBOX database — for verifying migrations without touching production (AGENTS.md §12).
#
# Completely separate from the production stack (`docker-compose.yml` / `scripts/db-*.sh`): its
# own compose project, container, volume, and host port (5434, vs production's 5433). Config
# comes from `.env.sandbox`. It can never reach the owner's real database. Prefer the
# `dev-dash sandbox …` wrapper; this script is what it calls.
#
#   scripts/sandbox-db.sh up       start the sandbox postgres (persistent until reset)
#   scripts/sandbox-db.sh down     stop it (volume preserved)
#   scripts/sandbox-db.sh reset    wipe its volume + start fresh (throwaway — no confirmation)
#   scripts/sandbox-db.sh psql     open a psql shell against it
#   scripts/sandbox-db.sh migrate  build + apply migrations headlessly against it, then exit
#   scripts/sandbox-db.sh url      print the sandbox DATABASE_URL
set -euo pipefail

# This script lives at static/scripts/, so the repo root is two levels up.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

COMPOSE_FILE="static/docker/docker-compose.sandbox.yml"
ENV_FILE=".env.sandbox"
PROJECT="devdash-sandbox"
CONTAINER="devdash-sandbox-postgres"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "error: $ENV_FILE not found — the sandbox needs its own env file (see AGENTS.md §12)." >&2
  exit 1
fi
# Load the sandbox config (POSTGRES_PORT, DATABASE_URL, …) into the environment.
set -a
# shellcheck disable=SC1090
source "$REPO_ROOT/$ENV_FILE"
set +a
PORT="${POSTGRES_PORT:-5434}"

# Hard safety rail: the sandbox must target its OWN port, never production's. If .env.sandbox
# were ever misconfigured to point elsewhere, refuse rather than risk the real database.
if [[ "${DATABASE_URL:-}" != *":${PORT}/"* ]]; then
  echo "error: DATABASE_URL in $ENV_FILE does not target the sandbox port ${PORT}. Refusing." >&2
  exit 1
fi

compose() { docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" -p "$PROJECT" "$@"; }

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker is not installed or not on PATH." >&2
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "error: the docker daemon is not running. Start Docker Desktop and retry." >&2
    exit 1
  fi
}

wait_healthy() {
  echo "waiting for the sandbox postgres to become healthy..."
  local status
  for _ in $(seq 1 30); do
    status="$(docker inspect -f '{{.State.Health.Status}}' "$CONTAINER" 2>/dev/null || echo starting)"
    if [[ "$status" == "healthy" ]]; then
      echo "sandbox postgres is healthy on port ${PORT}."
      return 0
    fi
    sleep 1
  done
  echo "error: sandbox postgres did not become healthy in time. Logs: docker compose -f $COMPOSE_FILE -p $PROJECT logs postgres" >&2
  return 1
}

sandbox_up() { require_docker; echo "starting the sandbox postgres (port ${PORT})..."; compose up -d; wait_healthy; }
sandbox_down() { require_docker; echo "stopping the sandbox postgres (volume preserved)..."; compose down --remove-orphans; }
sandbox_reset() {
  require_docker
  echo "wiping the sandbox volume and starting fresh..."
  compose down --remove-orphans --volumes
  compose up -d
  wait_healthy
}
sandbox_psql() { require_docker; compose exec postgres psql -U "${POSTGRES_USER:-devdash}" -d "${POSTGRES_DB:-devdash}"; }

# Build + run the app's REAL migration path (system::db::connect_and_migrate) headlessly against
# the sandbox, via the DEVDASH_MIGRATE_CHECK gate in main.rs. Exits non-zero if migrations fail.
sandbox_migrate() {
  sandbox_up
  echo "building (debug)..."
  cargo build
  local bin
  bin="target/debug/$(grep -m1 '^name' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
  echo "applying migrations against the sandbox: ${DATABASE_URL}"
  DATABASE_URL="$DATABASE_URL" DEVDASH_MIGRATE_CHECK=1 RUST_LOG="${RUST_LOG:-my_dev_dashboard=info,warn}" "$bin"
}

case "${1:-}" in
  up)      sandbox_up ;;
  down)    sandbox_down ;;
  reset)   sandbox_reset ;;
  psql)    sandbox_psql ;;
  migrate) sandbox_migrate ;;
  url)     echo "$DATABASE_URL" ;;
  *)       echo "usage: scripts/sandbox-db.sh {up|down|reset|psql|migrate|url}" >&2; exit 1 ;;
esac
