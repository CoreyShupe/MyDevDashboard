-- Terminal stages (e.g. "Complete", "Cancelled") are end states. On the board they collapse
-- to a ticket COUNT instead of listing their cards (revealable with "View tickets" to move a
-- ticket back out), and their tickets are excluded from the Notes "Add to ticket" picker.
-- Tickets can still be dragged INTO a terminal stage.
ALTER TABLE stages ADD COLUMN IF NOT EXISTS terminal BOOLEAN NOT NULL DEFAULT FALSE;
