//! Custom Askama template filters.

#![allow(clippy::unnecessary_wraps)]

use std::fmt::Display;

use chrono::{DateTime, Utc};

/// Returns the current year.
///
/// Usage in templates: `{{ ""|current_year }}`
#[allow(clippy::unnecessary_wraps)]
#[askama::filter_fn]
pub fn current_year(_value: impl Display, _env: &dyn askama::Values) -> askama::Result<i32> {
    use chrono::Datelike;
    Ok(chrono::Utc::now().year())
}

/// Humanize a datetime to a relative or absolute format.
///
/// Usage in templates: `{{ some_datetime|humanize_datetime }}`
#[askama::filter_fn]
pub fn humanize_datetime(dt: &DateTime<Utc>, _env: &dyn askama::Values) -> askama::Result<String> {
    let now = Utc::now();
    let duration = now.signed_duration_since(*dt);

    // Future dates
    if duration.num_seconds() < 0 {
        let future = dt.signed_duration_since(now);
        if future.num_days() > 7 {
            return Ok(dt.format("%b %d, %Y").to_string());
        } else if future.num_days() > 1 {
            return Ok(format!("in {} days", future.num_days()));
        } else if future.num_days() == 1 {
            return Ok("tomorrow".to_string());
        } else if future.num_hours() > 1 {
            return Ok(format!("in {} hours", future.num_hours()));
        } else if future.num_minutes() > 1 {
            return Ok(format!("in {} minutes", future.num_minutes()));
        }
        return Ok("in a moment".to_string());
    }

    // Past dates
    if duration.num_days() > 30 {
        Ok(dt.format("%b %d, %Y").to_string())
    } else if duration.num_days() > 1 {
        Ok(format!("{} days ago", duration.num_days()))
    } else if duration.num_days() == 1 {
        Ok("yesterday".to_string())
    } else if duration.num_hours() > 1 {
        Ok(format!("{} hours ago", duration.num_hours()))
    } else if duration.num_minutes() > 1 {
        Ok(format!("{} minutes ago", duration.num_minutes()))
    } else {
        Ok("just now".to_string())
    }
}

/// Humanize a datetime string (ISO 8601) to a relative or absolute format.
///
/// Usage in templates: `{{ some_datetime_string|humanize_datetime_str }}`
#[askama::filter_fn]
pub fn humanize_datetime_str(dt_str: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    // Parse ISO 8601 datetime string
    let dt = match DateTime::parse_from_rfc3339(dt_str) {
        Ok(parsed) => parsed.with_timezone(&Utc),
        Err(_) => return Ok(dt_str.to_string()), // Return as-is if parsing fails
    };

    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    // Future dates
    if duration.num_seconds() < 0 {
        let future = dt.signed_duration_since(now);
        if future.num_days() > 7 {
            return Ok(dt.format("%b %d, %Y").to_string());
        } else if future.num_days() > 1 {
            return Ok(format!("in {} days", future.num_days()));
        } else if future.num_days() == 1 {
            return Ok("tomorrow".to_string());
        } else if future.num_hours() > 1 {
            return Ok(format!("in {} hours", future.num_hours()));
        } else if future.num_minutes() > 1 {
            return Ok(format!("in {} minutes", future.num_minutes()));
        }
        return Ok("in a moment".to_string());
    }

    // Past dates
    if duration.num_days() > 30 {
        Ok(dt.format("%b %d, %Y").to_string())
    } else if duration.num_days() > 1 {
        Ok(format!("{} days ago", duration.num_days()))
    } else if duration.num_days() == 1 {
        Ok("yesterday".to_string())
    } else if duration.num_hours() > 1 {
        Ok(format!("{} hours ago", duration.num_hours()))
    } else if duration.num_minutes() > 1 {
        Ok(format!("{} minutes ago", duration.num_minutes()))
    } else {
        Ok("just now".to_string())
    }
}

/// Extract the numeric ID from a Shopify GID.
///
/// Examples:
/// - `gid://shopify/Collection/123` -> `123`
/// - `gid://shopify/Product/456` -> `456`
/// - `123` -> `123` (already numeric)
///
/// Usage in templates: `{{ id|extract_id }}`
#[askama::filter_fn]
pub fn extract_id(gid: &str, _env: &dyn askama::Values) -> askama::Result<String> {
    Ok(gid.split('/').next_back().unwrap_or(gid).to_string())
}
