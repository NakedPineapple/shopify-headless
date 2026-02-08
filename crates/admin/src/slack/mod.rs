//! Slack integration - re-exported from services crate.
//!
//! The Slack client, types, and message builders live in `naked-pineapple-services`.
//! Admin-specific webhook handling remains in the routes module.

pub use naked_pineapple_services::slack::*;
