-- A per-project **teardown script**: the mirror of the setup script (0008). A bash script the
-- owner can attach to a project, run automatically inside each worktree *right before it is
-- removed* (e.g. `docker compose down`, stop a dev server, drop a scratch database) so removing a
-- worktree cleans up whatever the setup script (or the owner) stood up. Durable identity (like
-- name + path + setup_script), so it lives on the `projects` row and is scoped to a profile through
-- it (AGENTS.md §9, §10).
--
-- Additive + safe (AGENTS.md §12): a new TEXT column with a DEFAULT, so existing rows get an
-- empty script (= "no teardown script") and no data is touched.
ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS teardown_script TEXT NOT NULL DEFAULT '';
