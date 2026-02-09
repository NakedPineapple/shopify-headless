SET search_path TO admin, public;

-- Mark any remaining 'queued' outbound emails as cancelled since
-- customer emails now go through Klaviyo and internal emails via SMTP.
UPDATE admin.outbound_email_queue
SET status = 'cancelled',
    error_message = 'Migrated to Klaviyo events and SMTP delivery'
WHERE status = 'queued';
