SET search_path TO admin, public;

-- Reverse the cancellation (cannot truly undo — re-queues cancelled rows).
UPDATE admin.outbound_email_queue
SET status = 'queued',
    error_message = NULL
WHERE status = 'cancelled'
  AND error_message = 'Migrated to Klaviyo events and SMTP delivery';
