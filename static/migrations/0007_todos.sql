-- Todos: quick, easy tasks the owner just needs to remember — lighter than a ticket, and
-- distinct from an uncategorized note (a todo is something to DO, so it carries a `done` flag).
-- Lives on its own "Todos" tab, works like the Notes list, and is profile-scoped (AGENTS.md §9).
CREATE TABLE IF NOT EXISTS todos (
    id         UUID        PRIMARY KEY,
    profile_id UUID        NOT NULL REFERENCES profiles (id) ON DELETE CASCADE,
    body       TEXT        NOT NULL,
    done       BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Listing order: open todos first, newest capture on top within each group.
CREATE INDEX IF NOT EXISTS idx_todos_profile ON todos (profile_id, done, created_at DESC);
