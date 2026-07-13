-- Verify all chess SQL functions are marked IMMUTABLE (none volatile/stable).
SELECT proname, provolatile = 'i' AS is_immutable
FROM pg_proc
WHERE pronamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'pg_chess')
  AND proname LIKE 'chess_%'
ORDER BY proname;
