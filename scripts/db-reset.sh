#!/usr/bin/env bash
# DESTRUCTIVE: stop the DB and DELETE the persistent volume, wiping all data.
# Use only when you want a clean slate. Requires explicit confirmation.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker

echo "WARNING: this will permanently delete the '$VOLUME_NAME' volume and ALL dashboard data."
read -r -p "Type 'reset' to confirm: " reply
if [[ "$reply" != "reset" ]]; then
  echo "aborted. Nothing was deleted."
  exit 0
fi

docker compose down
docker volume rm "$VOLUME_NAME" >/dev/null 2>&1 || true
echo "volume removed. Run ./scripts/db-up.sh to start fresh."
