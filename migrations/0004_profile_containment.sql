-- Multi-profile containment: each profile is a SELF-CONTAINED workspace. Everything belongs
-- to exactly one profile, and profiles never mix. Scoping:
--   profile → stages → tickets → notes   (tickets/ticket-notes inherit their profile via the
--   profile → uncategorized_notes         parent; stages & uncategorized notes carry it directly)
-- Deleting a profile cascades its entire workspace.
--
-- Schema only (no seed rows — AGENTS.md §5). The app fills `profile_id` on every insert and
-- flips `is_active` via the ProfileService; a DB migrated from the single-profile era keeps its
-- rows (their `profile_id` stays NULL and is simply not shown — `./scripts/db-reset.sh` for a
-- clean multi-profile slate).

-- Exactly one profile is "active" (the one whose workspace is shown). The partial unique index
-- allows any number of inactive rows but at most one active; the app sets it atomically with
-- `UPDATE profiles SET is_active = (id = $1)`.
ALTER TABLE profiles ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT FALSE;
CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_one_active ON profiles (is_active) WHERE is_active;

-- Stages belong to a profile directly.
ALTER TABLE stages ADD COLUMN IF NOT EXISTS profile_id UUID REFERENCES profiles (id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_stages_profile ON stages (profile_id);

-- Uncategorized notes belong to a profile directly.
ALTER TABLE uncategorized_notes ADD COLUMN IF NOT EXISTS profile_id UUID REFERENCES profiles (id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_uncategorized_notes_profile ON uncategorized_notes (profile_id);
