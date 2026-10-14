#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
CONTAINER="${1:-blindall-server-1}"

tar -cf - -C storage/seed . | docker exec -i "$CONTAINER" tar -xf - -C /app/storage
echo "seeded $(find storage/seed -type f | wc -l) files into $CONTAINER:/app/storage"
