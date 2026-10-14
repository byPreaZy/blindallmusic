-- =============================================================================
-- Row Level Security for the `public` schema.
--
-- The supabase/postgres image grants SELECT/INSERT/UPDATE/DELETE on public
-- tables to the PostgREST roles (anon, authenticated) via default privileges.
-- The Blind'All Music API is the authorization layer — the app connects as the
-- postgres superuser which bypasses RLS — so we enable RLS on every app table
-- and grant full access to service_role only. anon/authenticated REST calls
-- therefore get nothing until an explicit policy is added.
--
-- Guarded: if the Supabase roles don't exist (plain PostgreSQL), RLS is still
-- enabled and the policy step is skipped.
-- =============================================================================

DO $$
DECLARE
    t RECORD;
BEGIN
    FOR t IN
        SELECT c.relname AS name
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relkind = 'r'
          AND c.relname <> '_sqlx_migrations'
    LOOP
        EXECUTE format('ALTER TABLE public.%I ENABLE ROW LEVEL SECURITY', t.name);
    END LOOP;
END;
$$;

DO $$
DECLARE
    t RECORD;
BEGIN
    FOR t IN
        SELECT c.relname AS name
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public'
          AND c.relkind = 'r'
          AND c.relname <> '_sqlx_migrations'
    LOOP
        EXECUTE format(
            'CREATE POLICY service_role_all ON public.%I TO service_role '
            'USING (true) WITH CHECK (true)',
            t.name
        );
    END LOOP;
EXCEPTION
    WHEN undefined_object THEN
        -- service_role doesn't exist (non-Supabase Postgres): RLS stays
        -- enabled; postgres/superuser access is unaffected.
        RAISE NOTICE 'service_role role not found — RLS enabled without policies';
END;
$$;
