#!/usr/bin/env bash
# DESTRUCTIVE: down → wipe the persistent volume → up. Leaves a FRESH, running database (the
# app applies migrations on its next launch). Requires explicit confirmation.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker

echo "WARNING: this will permanently delete the '$VOLUME_NAME' volume and ALL dashboard data."
read -r -p "Type 'reset' to confirm: " reply
if [[ "$reply" != "reset" ]]; then
  echo "aborted. Nothing was deleted."
  exit 0
fi

# down → wipe → up, reusing the shared helpers so this stays in step with db-down / db-up.
echo "stopping the database..."
stop_db
echo "deleting volume '$VOLUME_NAME'..."
docker volume rm "$VOLUME_NAME" >/dev/null 2>&1 || true
echo "starting a fresh database..."
start_db
echo "done. Fresh database is up; the app applies migrations on its next launch."
