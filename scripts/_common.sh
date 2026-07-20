#!/usr/bin/env bash
# Shared helpers for the db-* scripts. Not meant to be run directly.
set -euo pipefail

# Resolve repo root regardless of where the script is invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

VOLUME_NAME="my-dev-dash-pgdata"
CONTAINER_NAME="my-dev-dash-postgres"

# Pin the compose project name so it is STABLE regardless of the checkout directory's name.
# Compose otherwise derives the project from the directory basename, so a directory rename
# would orphan the running container (compose would try to create a fresh one and collide on
# the fixed container name). Pinning it (matching `name:` in docker-compose.yml) prevents that.
PROJECT_NAME="my-dev-dash"
export COMPOSE_PROJECT_NAME="$PROJECT_NAME"

# Load .env if present so POSTGRES_* and DATABASE_URL are available to compose.
if [[ -f "$REPO_ROOT/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$REPO_ROOT/.env"
  set +a
else
  echo "note: no .env found; copying .env.example -> .env (edit it if needed)"
  cp "$REPO_ROOT/.env.example" "$REPO_ROOT/.env"
  set -a
  # shellcheck disable=SC1091
  source "$REPO_ROOT/.env"
  set +a
fi

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker is not installed or not on PATH." >&2
    echo "       install Docker Desktop for macOS: https://www.docker.com/products/docker-desktop/" >&2
    exit 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "error: the docker daemon is not running. Start Docker Desktop and retry." >&2
    exit 1
  fi
}

ensure_volume() {
  # Create the persistent named volume once. Idempotent.
  if ! docker volume inspect "$VOLUME_NAME" >/dev/null 2>&1; then
    echo "creating persistent volume '$VOLUME_NAME' (survives docker prune)..."
    docker volume create "$VOLUME_NAME" >/dev/null
  fi
}

# Remove a container that has our fixed name but is NOT owned by our compose project — e.g.
# left behind by an older project name after a directory rename. This is lossless: the
# database lives in the named volume "$VOLUME_NAME", which is untouched, so compose can
# recreate the container cleanly and re-attach the same data.
clear_stale_container() {
  docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1 || return 0
  local owner
  owner="$(docker inspect -f '{{ index .Config.Labels "com.docker.compose.project" }}' \
    "$CONTAINER_NAME" 2>/dev/null || true)"
  if [[ "$owner" != "$PROJECT_NAME" ]]; then
    echo "note: removing a stale '$CONTAINER_NAME' container (compose project '${owner:-none}'," \
         "not '$PROJECT_NAME'). Data is safe in volume '$VOLUME_NAME'; it will be recreated."
    docker rm -f "$CONTAINER_NAME" >/dev/null
  fi
}

# Stop and remove the postgres container (via compose, plus any same-named leftover). The
# named data volume is NEVER touched here, so this is lossless. `db-down` calls it on its own;
# `db-reset` calls it first, then deletes the volume.
stop_db() {
  docker compose down --remove-orphans >/dev/null 2>&1 || true
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}

# Bring the database up: ensure the volume exists, clear any stale (differently-owned)
# container, start via compose, and wait until healthy. `db-up` calls this; `db-reset` calls
# it last (after wiping the volume) to leave a fresh DB running. Returns non-zero on timeout.
start_db() {
  ensure_volume
  clear_stale_container
  echo "starting postgres via docker compose..."
  docker compose up -d
  echo "waiting for postgres to become healthy..."
  local status
  for _ in $(seq 1 30); do
    status="$(docker inspect -f '{{.State.Health.Status}}' "$CONTAINER_NAME" 2>/dev/null || echo starting)"
    if [[ "$status" == "healthy" ]]; then
      echo "postgres is healthy and listening on port ${POSTGRES_PORT:-5433}."
      echo "DATABASE_URL=${DATABASE_URL}"
      return 0
    fi
    sleep 1
  done
  echo "error: postgres did not become healthy in time. Check logs with: docker compose logs postgres" >&2
  return 1
}
