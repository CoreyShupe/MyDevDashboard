#!/usr/bin/env bash
# Start (and create, if needed) the local PostgreSQL instance with a persistent volume.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker
ensure_volume

echo "starting postgres via docker compose..."
docker compose up -d

echo "waiting for postgres to become healthy..."
for _ in $(seq 1 30); do
  status="$(docker inspect -f '{{.State.Health.Status}}' macdevdash_postgres 2>/dev/null || echo starting)"
  if [[ "$status" == "healthy" ]]; then
    echo "postgres is healthy and listening on port ${POSTGRES_PORT:-5433}."
    echo "DATABASE_URL=${DATABASE_URL}"
    exit 0
  fi
  sleep 1
done

echo "error: postgres did not become healthy in time. Check logs with: docker compose logs postgres" >&2
exit 1
