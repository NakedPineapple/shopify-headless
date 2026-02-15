-- TikTok Shop performance metrics snapshots.

SET search_path TO admin, public;

CREATE TABLE admin.tiktok_shop_performance (
    id                          SERIAL PRIMARY KEY,
    snapshot_date               DATE NOT NULL UNIQUE,
    on_time_delivery_rate       NUMERIC,
    late_dispatch_rate          NUMERIC,
    seller_fault_cancel_rate    NUMERIC,
    customer_satisfaction_rate  NUMERIC,
    overall_health              TEXT NOT NULL DEFAULT 'healthy',
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER tiktok_shop_performance_updated_at
    BEFORE UPDATE ON admin.tiktok_shop_performance
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
