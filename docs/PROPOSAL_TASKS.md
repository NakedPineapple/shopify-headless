# Proposal — Remaining Tasks

Features described in `crates/admin/templates/proposal/` that are not yet implemented.

---

## Customer Support

- [ ] **Live chat widget (storefront)** — Add a real-time chat widget to the storefront so customers can message support directly. The proposal describes AI-powered instant answers for common questions (order status, returns policy, etc.) with handoff to a human when needed.
- [ ] **Unified email inbox UI (admin)** — Build an admin interface for viewing and responding to inbound customer emails. The backend exists — `crates/automations/src/triage/` classifies emails and `admin.inbound_email` stores them with status, classification, and response drafts — but there is no admin route or template for the inbox.
- [ ] **Customer ticketing UI (admin)** — Surface triage results as a ticket queue in the admin panel with status tracking (open / waiting / resolved), assignment, and reply composition.
- [ ] **AI auto-response sending** — The triage pipeline populates `inbound_email.response_draft`, but nothing sends the draft automatically. Wire up an approval-and-send flow (possibly through Slack confirmation) so routine questions can be answered without manual intervention.

## AI Assistant

- [ ] **Document upload & storage** — The proposal lists "Your Documents" as one of 15 AI domains (product guides, vendor agreements, SOPs). No upload endpoint, file storage, or document table exists today.
- [ ] **Document search tool** — Add a Claude tool that retrieves relevant chunks from uploaded documents so the assistant can reference internal knowledge. All 126 current tools are Shopify-specific.
- [ ] **Slack approve / deny buttons** — The `pending_actions` table and `action_queue` service exist, but the Slack message with interactive Approve / Deny buttons and the callback handler to execute or discard the action need to be completed and verified end-to-end.

## Analytics

- [ ] **Revenue attribution dashboard** — All 10 ad-tracking pixels fire on the storefront, but there is no admin view that aggregates conversion data to show which channels drive revenue.
- [ ] **Advanced sales dashboard** — The current dashboard shows headline metrics. Add date-range filtering, time-series trend charts, and per-channel breakdowns as described in the proposal.
- [ ] **Profit margin reporting** — Manufacturing batch costs and inventory lots are tracked (`admin.manufacturing_batch`, `admin.inventory_lot`). Build a view that calculates and displays per-product and per-order profit margins using COGS from lot allocations.

## Automation

- [ ] **Daily / weekly business summary emails** — Automated reports compiling revenue, order count, top products, marketing performance, and inventory alerts. No scheduled job or template exists.
- [ ] **Cross-channel product sync** — The proposal claims edits in the admin panel propagate to Shopify and onward to every connected sales channel. Today the admin reads from Shopify (source of truth) but does not push changes back through admin-initiated writes that fan out to other channels.

## Storefront

- [ ] **Star ratings & product reviews** — No review submission, storage, or display system exists. The proposal shows star ratings on product cards and detail pages.
- [ ] **Customer Hub / self-service portal** — Account pages exist (orders, subscriptions, addresses), but the proposal describes a dedicated hub with searchable FAQ, AI-powered instant answers, and a support-ticket view. This goes beyond the current account section.


## External Integrations

~~~- [ ] **Mosyle (Fuse) device management** — Listed in the cost table (`_cost.html`) and features grid (`_features.html`) but no integration code exists.~~~
- [ ] **Amazon Seller Central** — Mentioned as a connected sales channel; no API client or sync logic implemented.
- [ ] **Faire wholesale marketplace** — Mentioned as a connected sales channel; no API client or sync logic implemented.

