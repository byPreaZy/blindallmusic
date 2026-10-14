#!/usr/bin/env bash
# Seed the `storage_data` volume with the room cover files under
# `storage/seed/` (collected official assets — pgloader migrated rows,
# not files). Idempotent: existing files are overwritten in place.
#
#   tools/seed_storage.sh [container]
#
# Default container: blindall-server-1.
set -euo pipefail
cd "$(dirname "$0")/.."
CONTAINER="${1:-blindall-server-1}"

tar -cf - -C storage/seed . | docker exec -i "$CONTAINER" tar -xf - -C /app/storage
echo "seeded $(find storage/seed -type f | wc -l) files into $CONTAINER:/app/storage"
