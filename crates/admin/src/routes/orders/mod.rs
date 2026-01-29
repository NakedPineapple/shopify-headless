//! Orders management route handlers.
//!
//! This module contains handlers for order listing, detail views, bulk actions,
//! single order actions, printing, and order editing.

mod actions;
mod bulk;
mod detail;
mod edit;
mod list;
mod print;
pub mod types;

// Re-export types needed by templates and router
pub use types::{OrderColumnVisibility, OrderDetailView, OrderTableView, OrderView, OrdersQuery};

// Re-export list handlers
pub use list::{OrdersIndexTemplate, index};

// Re-export detail handlers
pub use detail::{
    CancelFormInput, NoteFormInput, OrderShowTemplate, cancel, mark_paid, show, update_note,
};

// Re-export bulk handlers
pub use bulk::{
    BulkOrdersInput, BulkTagsInput, bulk_add_tags, bulk_archive, bulk_cancel, bulk_remove_tags,
};

// Re-export single action handlers
pub use actions::{
    ArchiveParams, CaptureInput, FulfillInput, HoldInput, RefundInput, ReturnInput, TagInput,
    archive, calculate_refund, capture, create_return, fulfill, hold_fulfillment, refund,
    release_hold, update_tags,
};

// Re-export print handlers
pub use print::{
    OrderInvoiceTemplate, OrderPackingSlipTemplate, PrintLineItemView, PrintOrderView, PrintQuery,
    print,
};

// Re-export edit handlers
pub use edit::{
    AddCustomItemInput, AddDiscountInput, AddShippingInput, AddVariantInput, CommitEditInput,
    EditLineItemsPartial, EditSummaryPartial, OrderEditTemplate, ProductSearchQuery,
    RemoveDiscountInput, RemoveShippingInput, SetQuantityInput, UpdateDiscountInput,
    UpdateShippingInput, edit, edit_add_custom_item, edit_add_discount, edit_add_shipping,
    edit_add_variant, edit_commit, edit_discard, edit_remove_discount, edit_remove_shipping,
    edit_search_products, edit_set_quantity, edit_update_discount, edit_update_shipping,
};

// Re-export additional types from types module that might be needed externally
pub use types::{
    AddressView, EditLineItemView, EditShippingLineView, FulfilledLineItemView,
    FulfillmentOrderLineItemView, FulfillmentOrderView, FulfillmentView, LineItemView,
    OrderEditView, RiskView, TimelineEventView, TransactionView,
};
