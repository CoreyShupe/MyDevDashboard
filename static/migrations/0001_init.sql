-- Initial schema for the Mac Dev Dashboard.
-- Applied automatically at app startup via sqlx::migrate!.

-- The owner's profile. This is a single-user tool, but we key by id for cleanliness.
CREATE TABLE IF NOT EXISTS profiles (
    id           UUID PRIMARY KEY,
    display_name TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Configurable ticket stages (e.g. "Pending", "In Progress", "Complete").
-- `position` gives left-to-right column ordering on the board.
CREATE TABLE IF NOT EXISTS stages (
    id         UUID PRIMARY KEY,
    name       TEXT        NOT NULL,
    position   INTEGER     NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_stages_position ON stages (position);

-- Tickets belong to exactly one stage. `position` orders them within a column.
CREATE TABLE IF NOT EXISTS tickets (
    id          UUID PRIMARY KEY,
    stage_id    UUID        NOT NULL REFERENCES stages (id) ON DELETE CASCADE,
    title       TEXT        NOT NULL,
    description TEXT        NOT NULL DEFAULT '',
    position    INTEGER     NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tickets_stage ON tickets (stage_id, position);

-- Free-text notes attached to a ticket.
CREATE TABLE IF NOT EXISTS notes (
    id         UUID PRIMARY KEY,
    ticket_id  UUID        NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    body       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_notes_ticket ON notes (ticket_id, created_at);
