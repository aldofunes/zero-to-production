-- Add migration script here
BEGIN;

-- backfill 'status' for historical entries
UPDATE subscriptions
SET "status" = 'confirmed'
WHERE "status" IS NULL;

-- make 'status' mandatory
ALTER TABLE subscriptions
ALTER COLUMN "status"
SET NOT NULL;

COMMIT;