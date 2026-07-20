-- Remember each profile's last-viewed workspace page, so switching profiles or relaunching the
-- app restores where the owner left off (AGENTS.md §9). One value per profile — `profiles`
-- already has one row each, so this is just a column on it.
--
-- Additive + defaulted: existing rows backfill to the main dashboard ('tasks'), which is also
-- where a freshly-created profile lands. Anything unrecognized is read back as the default too
-- (see `ProfileView::from_db`), so a stray value never breaks navigation. Schema only — no seed
-- rows (AGENTS.md §5).
ALTER TABLE profiles ADD COLUMN IF NOT EXISTS last_view TEXT NOT NULL DEFAULT 'tasks';
