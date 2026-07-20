-- Uncategorized notes: quick free-text captures that live on their own "Notes" tab until
-- they're filed — turned into a new ticket, or added onto an existing one.
--
-- Deliberately minimal for now (id + body + created_at). Notes may grow more structure
-- later (tags, pinning, links back to what they became); add those via a new migration.
CREATE TABLE IF NOT EXISTS uncategorized_notes (
    id         UUID PRIMARY KEY,
    body       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Newest-first is the default listing order (most-recent capture on top).
CREATE INDEX IF NOT EXISTS idx_uncategorized_notes_created ON uncategorized_notes (created_at DESC);
