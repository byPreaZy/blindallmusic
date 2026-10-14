-- Postgres: explicit-id inserts (seeds / imported MySQL data) don't advance
-- GENERATED ... AS IDENTITY sequences — resync every identity column to MAX(id).
DO $$
DECLARE
    r RECORD;
    v_max BIGINT;
BEGIN
    FOR r IN
        SELECT c.table_name, c.column_name, pg_get_serial_sequence(
                   format('%I.%I', c.table_schema, c.table_name), c.column_name) AS seq
        FROM information_schema.columns c
        WHERE c.table_schema = 'public'
          AND c.is_identity = 'YES'
    LOOP
        IF r.seq IS NOT NULL THEN
            EXECUTE format('SELECT COALESCE(MAX(%I), 0) FROM %I', r.column_name, r.table_name)
                INTO v_max;
            EXECUTE format('SELECT setval(%L, GREATEST(%s, 1), %s > 0)', r.seq, v_max, v_max);
        END IF;
    END LOOP;
END $$;
