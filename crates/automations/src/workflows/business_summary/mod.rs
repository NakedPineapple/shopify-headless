//! Daily and weekly business summary email workflows.
//!
//! Collects revenue, order, expense, and inventory data then renders
//! branded HTML + plain text emails and delivers them via SMTP.

pub mod daily;
pub mod data;
pub mod templates;
pub mod types;
pub mod weekly;
