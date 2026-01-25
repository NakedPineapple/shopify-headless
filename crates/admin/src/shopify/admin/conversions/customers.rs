//! Customer type conversion functions.

use crate::shopify::types::{
    Address, Customer, CustomerConnection, CustomerState, Money, PageInfo,
};

use super::super::queries::{get_customer, get_customers};
use super::currency_code_to_string;

// =============================================================================
// GetCustomer conversions
// =============================================================================

pub fn convert_customer(customer: get_customer::GetCustomerCustomer) -> Customer {
    let state = match customer.state {
        get_customer::CustomerState::ENABLED => CustomerState::Enabled,
        get_customer::CustomerState::INVITED => CustomerState::Invited,
        get_customer::CustomerState::DECLINED => CustomerState::Declined,
        get_customer::CustomerState::DISABLED | get_customer::CustomerState::Other(_) => {
            CustomerState::Disabled
        }
    };

    // Extract email and marketing consent info from defaultEmailAddress
    let (email, accepts_marketing, accepts_marketing_updated_at) = customer
        .default_email_address
        .as_ref()
        .map_or((None, false, None), |email_addr| {
            let accepts = matches!(
                email_addr.marketing_state,
                get_customer::CustomerEmailAddressMarketingState::SUBSCRIBED
            );
            (
                Some(email_addr.email_address.clone()),
                accepts,
                email_addr.marketing_updated_at.clone(),
            )
        });

    // Extract phone from defaultPhoneNumber
    let phone = customer
        .default_phone_number
        .as_ref()
        .map(|p| p.phone_number.clone());

    Customer {
        id: customer.id,
        email,
        first_name: customer.first_name,
        last_name: customer.last_name,
        display_name: customer.display_name,
        phone,
        state,
        accepts_marketing,
        accepts_marketing_updated_at,
        orders_count: customer.number_of_orders.parse().unwrap_or(0),
        total_spent: Money {
            amount: customer.amount_spent.amount,
            currency_code: currency_code_to_string(customer.amount_spent.currency_code),
        },
        note: customer.note,
        tags: customer.tags,
        default_address: customer.default_address.map(convert_address_single),
        addresses: customer
            .addresses_v2
            .edges
            .into_iter()
            .map(|e| convert_address_v2_node(e.node))
            .collect(),
        created_at: customer.created_at,
        updated_at: customer.updated_at,
    }
}

fn convert_address_single(a: get_customer::GetCustomerCustomerDefaultAddress) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}

fn convert_address_v2_node(a: get_customer::GetCustomerCustomerAddressesV2EdgesNode) -> Address {
    Address {
        address1: a.address1,
        address2: a.address2,
        city: a.city,
        province_code: a.province_code,
        country_code: a.country_code_v2.map(|c| format!("{c:?}")),
        zip: a.zip,
        first_name: a.first_name,
        last_name: a.last_name,
        company: a.company,
        phone: a.phone,
    }
}

// =============================================================================
// GetCustomers conversions
// =============================================================================

pub fn convert_customer_connection(
    conn: get_customers::GetCustomersCustomers,
) -> CustomerConnection {
    CustomerConnection {
        customers: conn
            .edges
            .into_iter()
            .map(|e| convert_customers_list_customer(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            start_cursor: conn.page_info.start_cursor,
            end_cursor: conn.page_info.end_cursor,
        },
    }
}

fn convert_customers_list_customer(
    customer: get_customers::GetCustomersCustomersEdgesNode,
) -> Customer {
    let state = match customer.state {
        get_customers::CustomerState::ENABLED => CustomerState::Enabled,
        get_customers::CustomerState::INVITED => CustomerState::Invited,
        get_customers::CustomerState::DECLINED => CustomerState::Declined,
        get_customers::CustomerState::DISABLED | get_customers::CustomerState::Other(_) => {
            CustomerState::Disabled
        }
    };

    // Extract email and marketing consent info from defaultEmailAddress
    let (email, accepts_marketing, accepts_marketing_updated_at) = customer
        .default_email_address
        .as_ref()
        .map_or((None, false, None), |email_addr| {
            let accepts = matches!(
                email_addr.marketing_state,
                get_customers::CustomerEmailAddressMarketingState::SUBSCRIBED
            );
            (
                Some(email_addr.email_address.clone()),
                accepts,
                email_addr.marketing_updated_at.clone(),
            )
        });

    // Extract phone from defaultPhoneNumber
    let phone = customer
        .default_phone_number
        .as_ref()
        .map(|p| p.phone_number.clone());

    Customer {
        id: customer.id,
        email,
        first_name: customer.first_name,
        last_name: customer.last_name,
        display_name: customer.display_name,
        phone,
        state,
        accepts_marketing,
        accepts_marketing_updated_at,
        orders_count: customer.number_of_orders.parse().unwrap_or(0),
        total_spent: Money {
            amount: customer.amount_spent.amount,
            currency_code: currency_code_to_string(customer.amount_spent.currency_code),
        },
        note: customer.note,
        tags: customer.tags,
        default_address: customer.default_address.map(|a| Address {
            address1: a.address1,
            address2: a.address2,
            city: a.city,
            province_code: a.province_code,
            country_code: a.country_code_v2.map(|c| format!("{c:?}")),
            zip: a.zip,
            first_name: a.first_name,
            last_name: a.last_name,
            company: a.company,
            phone: a.phone,
        }),
        addresses: customer
            .addresses_v2
            .edges
            .into_iter()
            .map(|e| Address {
                address1: e.node.address1,
                address2: e.node.address2,
                city: e.node.city,
                province_code: e.node.province_code,
                country_code: e.node.country_code_v2.map(|c| format!("{c:?}")),
                zip: e.node.zip,
                first_name: e.node.first_name,
                last_name: e.node.last_name,
                company: e.node.company,
                phone: e.node.phone,
            })
            .collect(),
        created_at: customer.created_at,
        updated_at: customer.updated_at,
    }
}
