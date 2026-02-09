-- Request board: Kanban-style bug reports and feature requests.

SET search_path TO admin, public;

-- Enum types
CREATE TYPE admin.request_type AS ENUM ('bug_report', 'feature_request');
CREATE TYPE admin.request_priority AS ENUM ('low', 'medium', 'high', 'critical');
CREATE TYPE admin.request_status AS ENUM ('new', 'under_review', 'in_progress', 'done', 'closed');

-- Main items table
CREATE TABLE admin.request_board_item (
    id              SERIAL PRIMARY KEY,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    request_type    admin.request_type NOT NULL,
    priority        admin.request_priority NOT NULL DEFAULT 'medium',
    status          admin.request_status NOT NULL DEFAULT 'new',
    position        INTEGER NOT NULL DEFAULT 0,
    created_by      INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_request_board_item_status_position ON admin.request_board_item(status, position);
CREATE INDEX idx_request_board_item_created_by ON admin.request_board_item(created_by);

CREATE TRIGGER request_board_item_updated_at
    BEFORE UPDATE ON admin.request_board_item
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Comments table
CREATE TABLE admin.request_board_comment (
    id              SERIAL PRIMARY KEY,
    item_id         INTEGER NOT NULL REFERENCES admin.request_board_item(id) ON DELETE CASCADE,
    admin_user_id   INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    body            TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_request_board_comment_item ON admin.request_board_comment(item_id);

CREATE TRIGGER request_board_comment_updated_at
    BEFORE UPDATE ON admin.request_board_comment
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
