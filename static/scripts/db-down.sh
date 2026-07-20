#!/usr/bin/env bash
# Stop the local PostgreSQL container. The persistent volume is preserved.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker
echo "stopping postgres (data volume '$VOLUME_NAME' is preserved)..."
stop_db
echo "done. Your data is safe in the named volume."
