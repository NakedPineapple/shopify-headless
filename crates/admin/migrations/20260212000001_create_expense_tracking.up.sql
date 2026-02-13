-- Expense tracking: categories and individual expense entries
SET search_path TO admin, public;

-- Enum for expense classification
CREATE TYPE admin.expense_type AS ENUM (
    'advertising', 'saas', 'shipping', 'labor', 'supplies', 'services', 'other'
);

-- Predefined + user-created expense categories
CREATE TABLE admin.expense_category (
    id          SERIAL PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    expense_type admin.expense_type NOT NULL,
    description TEXT,
    is_system   BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER update_expense_category_updated_at
    BEFORE UPDATE ON admin.expense_category
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

-- Individual expense entries
CREATE TABLE admin.expense (
    id                   SERIAL PRIMARY KEY,
    category_id          INTEGER NOT NULL REFERENCES admin.expense_category(id),
    description          TEXT NOT NULL,
    amount               DECIMAL(12,4) NOT NULL,
    currency_code        TEXT NOT NULL DEFAULT 'USD',
    expense_date         DATE NOT NULL,
    is_recurring         BOOLEAN NOT NULL DEFAULT false,
    recurrence_interval  TEXT CHECK (recurrence_interval IN ('monthly', 'quarterly', 'yearly')),
    recurrence_end_date  DATE,
    channel_name         TEXT,
    vendor               TEXT,
    notes                TEXT,
    created_by           INTEGER REFERENCES admin.admin_user(id),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER update_expense_updated_at
    BEFORE UPDATE ON admin.expense
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();

CREATE INDEX idx_expense_category_id ON admin.expense(category_id);
CREATE INDEX idx_expense_date ON admin.expense(expense_date);
CREATE INDEX idx_expense_channel ON admin.expense(channel_name) WHERE channel_name IS NOT NULL;

-- Seed system categories
INSERT INTO admin.expense_category (name, expense_type, description, is_system) VALUES
    -- SaaS platforms
    ('SaaS — Shopify',     'saas', 'Shopify subscription and transaction fees', true),
    ('SaaS — Fly.io',      'saas', 'Application hosting', true),
    ('SaaS — Cloudflare',  'saas', 'CDN, DNS, and R2 storage', true),
    ('SaaS — Tailscale',   'saas', 'VPN and network access', true),
    ('SaaS — Klaviyo',     'saas', 'Email marketing platform', true),
    ('SaaS — Anthropic',   'saas', 'Claude AI API usage', true),
    ('SaaS — OpenAI',      'saas', 'OpenAI API usage', true),
    ('SaaS — Sentry',      'saas', 'Error monitoring', true),
    ('SaaS — Better Stack','saas', 'Uptime monitoring and logging', true),
    ('SaaS — Mixpanel',    'saas', 'Product analytics', true),
    ('SaaS — Crazy Egg',   'saas', 'Heatmaps and session recording', true),
    -- Advertising platforms
    ('Advertising — Meta',      'advertising', 'Facebook and Instagram ads', true),
    ('Advertising — Google',    'advertising', 'Google Ads (Search, Display, Shopping)', true),
    ('Advertising — TikTok',    'advertising', 'TikTok ads', true),
    ('Advertising — Pinterest', 'advertising', 'Pinterest ads', true),
    ('Advertising — Snapchat',  'advertising', 'Snapchat ads', true),
    ('Advertising — Microsoft', 'advertising', 'Microsoft/Bing ads', true),
    ('Advertising — Twitter',   'advertising', 'Twitter/X ads', true),
    -- Other common categories
    ('Shipping',   'shipping', 'Shipping and fulfillment costs', true),
    ('Supplies',   'supplies', 'Packaging and office supplies', true),
    ('Services',   'services', 'Professional services and contractors', true),
    ('Labor',      'labor',    'Employee wages and benefits', true);
