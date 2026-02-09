//! Slack message builders for email triage notifications.
//!
//! Builds Block Kit messages for:
//! - Email review requests (with approve/reject buttons)
//! - Email notification alerts (no buttons)
//! - Approval/rejection confirmations

use naked_pineapple_services::slack::{
    ActionElement, Block, ButtonStyle, ContextElement, PlainText, Text,
};

use crate::triage::types::EmailClassification;

/// Build a review message for a classified email with approve/reject buttons.
#[must_use]
pub fn build_email_review_message(
    email_id: i32,
    from_address: &str,
    subject: &str,
    classification: EmailClassification,
    draft_preview: &str,
) -> Vec<Block> {
    let truncated_draft = if draft_preview.len() > 1500 {
        format!("{}...", &draft_preview[..1500])
    } else {
        draft_preview.to_string()
    };

    vec![
        Block::Header {
            text: PlainText::new("Email Response Review"),
        },
        Block::Section {
            text: Text::mrkdwn(format!(
                "*From:* {from_address}\n*Subject:* {subject}\n*Classification:* {}",
                classification.label()
            )),
            accessory: None,
        },
        Block::Divider,
        Block::Section {
            text: Text::mrkdwn(format!("*Draft Response:*\n{truncated_draft}")),
            accessory: None,
        },
        Block::Divider,
        Block::Actions {
            elements: vec![
                ActionElement::Button {
                    text: PlainText::new("Approve & Send"),
                    action_id: format!("email_approve_{email_id}"),
                    value: Some(email_id.to_string()),
                    style: Some(ButtonStyle::Primary),
                },
                ActionElement::Button {
                    text: PlainText::new("Reject"),
                    action_id: format!("email_reject_{email_id}"),
                    value: Some(email_id.to_string()),
                    style: Some(ButtonStyle::Danger),
                },
            ],
        },
    ]
}

/// Build a notification message for an email that doesn't need review.
#[must_use]
pub fn build_email_notification_message(
    from_address: &str,
    subject: &str,
    classification: EmailClassification,
    reasoning: &str,
) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("Inbound Email"),
        },
        Block::Section {
            text: Text::mrkdwn(format!(
                "*From:* {from_address}\n*Subject:* {subject}\n*Classification:* {}",
                classification.label()
            )),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("AI reasoning: {reasoning}"),
            }],
        },
    ]
}

/// Build an approved confirmation message (replaces the review message).
#[must_use]
pub fn build_email_approved_message(
    from_address: &str,
    subject: &str,
    approved_by: &str,
) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("Email Response Sent"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*From:* {from_address}\n*Subject:* {subject}")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Approved by *{approved_by}*"),
            }],
        },
    ]
}

/// Build a rejected confirmation message (replaces the review message).
#[must_use]
pub fn build_email_rejected_message(
    from_address: &str,
    subject: &str,
    rejected_by: &str,
) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("Email Response Rejected"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*From:* {from_address}\n*Subject:* {subject}")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Rejected by *{rejected_by}*"),
            }],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_message_has_buttons() {
        let blocks = build_email_review_message(
            42,
            "customer@example.com",
            "Where is my order?",
            EmailClassification::OrderInquiry,
            "Thank you for reaching out...",
        );

        // Should have Header, Section, Divider, Section, Divider, Actions
        assert_eq!(blocks.len(), 6);

        let last = blocks.last().expect("blocks");
        match last {
            Block::Actions { elements } => {
                assert_eq!(elements.len(), 2);
                let first = elements.first().expect("first element");
                match first {
                    ActionElement::Button { action_id, .. } => {
                        assert!(action_id.starts_with("email_approve_"));
                    }
                }
            }
            _ => panic!("expected Actions block"),
        }
    }

    #[test]
    fn test_notification_message_no_buttons() {
        let blocks = build_email_notification_message(
            "vendor@example.com",
            "Wholesale inquiry",
            EmailClassification::BusinessVendor,
            "Business partnership request",
        );

        // Should not contain Actions block
        for block in &blocks {
            assert!(!matches!(block, Block::Actions { .. }));
        }
    }

    #[test]
    fn test_review_message_truncates_long_draft() {
        let long_draft = "x".repeat(3000);
        let blocks = build_email_review_message(
            1,
            "test@test.com",
            "Test",
            EmailClassification::OrderInquiry,
            &long_draft,
        );

        // The draft section should be truncated
        if let Some(Block::Section { text, .. }) = blocks.get(3) {
            match text {
                Text::Mrkdwn { text } => {
                    assert!(text.len() < 2000);
                    assert!(text.ends_with("..."));
                }
                Text::PlainText { .. } => panic!("expected Mrkdwn text"),
            }
        }
    }
}
