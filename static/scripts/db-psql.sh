#!/usr/bin/env bash
# Open an interactive psql shell against the running database.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker
exec compose exec postgres psql -U "${POSTGRES_USER:-devdash}" -d "${POSTGRES_DB:-devdash}"
