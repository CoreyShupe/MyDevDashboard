-- A per-project **setup script**: a bash script the owner can attach to a project, run
-- automatically inside each freshly-created worktree (e.g. `bun install`, `npm ci`, `cargo build`)
-- so a new worktree is ready to work in. Durable identity (like name + path), so it lives on the
-- `projects` row and is scoped to a profile through it (AGENTS.md §9, §10).
--
-- Additive + safe (AGENTS.md §12): a new TEXT column with a DEFAULT, so existing rows get an
-- empty script (= "no setup script") and no data is touched.
ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS setup_script TEXT NOT NULL DEFAULT '';
