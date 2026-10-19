-- Resynchronize every serial/bigserial sequence with its table's MAX(id).
-- Imports and seeders that insert explicit ids can drift sequences ahead or
-- behind; a sequence behind MAX(id) makes every later INSERT fail with a
-- duplicate-key error on the PK (seen on track_answers). Idempotent.
DO $$
DECLARE
    rec RECORD;
    max_id BIGINT;
BEGIN
    FOR rec IN
        SELECT t.relname AS tbl, s.relname AS seq, a.attname AS col
        FROM pg_class s
        JOIN pg_depend d ON d.objid = s.oid
                    AND d.classid = 'pg_class'::regclass
                    AND d.refclassid = 'pg_class'::regclass
        JOIN pg_class t ON t.oid = d.refobjid
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = d.refobjsubid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        WHERE s.relkind = 'S' AND n.nspname = 'public'
    LOOP
        EXECUTE format('SELECT MAX(%I) FROM %I', rec.col, rec.tbl) INTO max_id;
        IF max_id IS NOT NULL THEN
            EXECUTE format('SELECT setval(%L, %s)', rec.seq, max_id);
        END IF;
    END LOOP;
END $$;
