-- Ticket hierarchy: a ticket may have a parent ticket (and thus children).
-- ON DELETE SET NULL: deleting a parent orphans its children (they become top-level)
-- rather than deleting them.

ALTER TABLE tickets
    ADD COLUMN IF NOT EXISTS parent_id UUID NULL REFERENCES tickets (id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tickets_parent ON tickets (parent_id);
