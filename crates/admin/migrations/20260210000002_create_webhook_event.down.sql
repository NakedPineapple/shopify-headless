REVOKE ALL ON admin.webhook_event FROM webhook_receiver;
REVOKE ALL ON SEQUENCE admin.webhook_event_id_seq FROM webhook_receiver;
REVOKE USAGE ON SCHEMA admin FROM webhook_receiver;
DROP ROLE IF EXISTS webhook_receiver;

DROP TABLE IF EXISTS admin.webhook_event;
