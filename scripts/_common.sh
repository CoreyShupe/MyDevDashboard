#!/usr/bin/env bash
# Shared helpers for the db-* scripts. Not meant to be run directly.
set -euo pipefail

# Resolve repo root regardless of where the script is invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

VOLUME_NAME="macdevdash_pgdata"

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
