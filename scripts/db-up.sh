#!/usr/bin/env bash
# Start (and create, if needed) the local PostgreSQL instance with a persistent volume.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

require_docker
start_db
