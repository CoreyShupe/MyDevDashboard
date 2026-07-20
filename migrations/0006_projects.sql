-- Projects & worktrees for the "Projects" tab.
--
-- A `project` is a LOCAL repository path the owner already has on disk (this tool never
-- clones — it points at existing repos). Git-derived facts (origin URL, branch, up-to-date
-- status) are NOT stored: they are read live from git at snapshot time, so they can never go
-- stale. Only the durable identity (name + path) lives here, scoped to a profile (AGENTS.md §9).
CREATE TABLE IF NOT EXISTS projects (
    id         UUID PRIMARY KEY,
    profile_id UUID        NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    path       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_projects_profile ON projects (profile_id);

-- Git worktrees living at {repo}/.github/worktrees/{name}. Each is tied 1:1 to a ticket AND a
-- project: a ticket has AT MOST ONE worktree per project (the UNIQUE below), and the `branch`
-- is shared across all of a ticket's worktrees (chosen once, on the first one). This is a
-- lightweight, churny table (common create/delete), so it carries only what it needs.
--
-- Deletion leaves a REMNANT rather than a hard delete: `removed_at` is set (and the on-disk
-- worktree cleaned up), keeping the branch name as a historical marker on the ticket so the
-- worktree can be recreated later. A live worktree has `removed_at IS NULL`. Reconciliation
-- flips a row to removed if its folder vanished from disk outside the app.
CREATE TABLE IF NOT EXISTS worktrees (
    id         UUID        PRIMARY KEY,
    project_id UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    ticket_id  UUID        NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    branch     TEXT        NOT NULL,
    removed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, ticket_id)
);

CREATE INDEX IF NOT EXISTS idx_worktrees_project ON worktrees (project_id);
CREATE INDEX IF NOT EXISTS idx_worktrees_ticket ON worktrees (ticket_id);

-- Two live worktrees in the same repo must not collide on their on-disk folder name; historical
-- remnants (removed_at set) are exempt so a name is reusable after removal.
CREATE UNIQUE INDEX IF NOT EXISTS uq_worktrees_active_name
    ON worktrees (project_id, name)
    WHERE removed_at IS NULL;
