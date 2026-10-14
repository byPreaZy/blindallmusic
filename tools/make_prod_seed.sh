#!/usr/bin/env bash
# Generate prod_seed.sql — official content only (no users, rooms, scores…).
# Run while the dev stack is up:
#   ./tools/make_prod_seed.sh [container] [admin_email] [admin_name]
# Output: ./prod_seed.sql  — apply AFTER migrations on a FRESH database:
#   psql "$DATABASE_URL" -f prod_seed.sql
set -euo pipefail
cd "$(dirname "$0")/.."

CONTAINER="${1:-blindall-db-1}"
ADMIN_EMAIL="${2:-clerelydesign@gmail.com}"
ADMIN_NAME="${3:-byPreaZy}"
DB="postgres"; PGUSER="postgres"; OUT="prod_seed.sql"

psqlc() { docker exec -i "$CONTAINER" psql -U "$PGUSER" -d "$DB" "$@"; }

cols() {
  psqlc -tAc "SELECT string_agg(column_name, ', ' ORDER BY ordinal_position)
              FROM information_schema.columns
              WHERE table_schema='public' AND table_name='$1'"
}

OFFICIAL_OWNER=7   # compte « Blind'All Music » (rooms officielles)

emit_copy() { # table, where
  local t="$1" w="$2" c
  c="$(cols "$t")"
  echo "COPY public.$t ($c) FROM stdin;"
  psqlc -c "\copy (SELECT $c FROM $t WHERE $w ORDER BY 1) TO STDOUT"
  printf '\\.\n\n'
}

ADMIN_PASS="$(psqlc -tAc "SELECT encode(gen_random_bytes(10), 'hex')" | tr -d '[:space:]')"
ADMIN_HASH="$(psqlc -tAc "SELECT crypt('${ADMIN_PASS}', gen_salt('bf', 12))" | tr -d '[:space:]')"
NEXT_USER_ID="$(psqlc -tAc "SELECT COALESCE(max(id),0)+1 FROM users WHERE id <> $OFFICIAL_OWNER" | tr -d '[:space:]')"
NEXT_USER_ID=$(( NEXT_USER_ID > OFFICIAL_OWNER ? NEXT_USER_ID : OFFICIAL_OWNER + 1 ))

{
  echo "-- Blind'All Music — prod seed (official content). Apply on fresh schema."
  echo "SET session_replication_role = replica;"
  echo

  emit_copy answer_types "true"
  emit_copy categories   "true"
  emit_copy faqs         "true"
  emit_copy pages        "true"

  emit_copy users     "id = $OFFICIAL_OWNER"
  emit_copy playlists "user_id = $OFFICIAL_OWNER"
  emit_copy rooms     "user_id = $OFFICIAL_OWNER AND is_public"
  emit_copy playlist_room "room_id IN (SELECT id FROM rooms WHERE user_id = $OFFICIAL_OWNER AND is_public)
                            AND playlist_id IN (SELECT id FROM playlists WHERE user_id = $OFFICIAL_OWNER)"
  emit_copy tracks    "playlist_id IN (SELECT id FROM playlists WHERE user_id = $OFFICIAL_OWNER)"
  emit_copy track_answers "track_id IN (SELECT id FROM tracks t WHERE t.playlist_id IN
                            (SELECT id FROM playlists WHERE user_id = $OFFICIAL_OWNER))"

  cat <<SQL
INSERT INTO public.users
  (id, team_id, name, email, email_verified_at, password, is_administrator,
   elo, is_guest, show_donation_history_on_profile, photo_path, eth_address,
   provider, provider_id, ip, guest_token, remember_token,
   created_at, updated_at, deleted_at)
VALUES ($NEXT_USER_ID, NULL, '$ADMIN_NAME', '$ADMIN_EMAIL', now(), '$ADMIN_HASH',
        true, 1500, false, false, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
        now(), now(), NULL);

-- resync sequences after explicit-id COPYs
SQL

  for t in users rooms playlists tracks track_answers categories answer_types faqs pages; do
    echo "SELECT setval(pg_get_serial_sequence('public.$t','id'), (SELECT COALESCE(max(id),1) FROM public.$t));"
  done

  cat <<'SQL'
SET session_replication_role = DEFAULT;

-- Migration 0003 seeds demo rows unconditionally (demo room + e2e admin with
-- a well-known password). Dev-only: cascade removes their rooms/playlists/
-- tracks/messages. Runs after role reset so FK cascades fire.
DELETE FROM public.users WHERE email IN ('demo@blindall.test', 'e2e-admin@blindall.test');
SQL
} > "$OUT"

echo "prod_seed.sql written — admin: $ADMIN_EMAIL / password: $ADMIN_PASS"
echo "(hash embedded; the plaintext above is shown once — save it now)"
