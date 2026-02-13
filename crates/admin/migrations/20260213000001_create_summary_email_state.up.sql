SET search_path TO admin, public;

CREATE TABLE admin.summary_email_state (
    workflow_name TEXT PRIMARY KEY,
    last_run_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER set_summary_email_state_updated_at
    BEFORE UPDATE ON admin.summary_email_state
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
