//! Cart type conversion functions.

use tracing::warn;

use crate::shopify::types::{
    Attribute, Cart, CartBuyerIdentity, CartCost, CartCustomer, CartDiscountCode, CartLine,
    CartLineCost, CartMerchandise, CartMerchandiseProduct, CartUserError, DiscountAllocation,
    Image, Money, SelectedOption,
};

use super::super::queries::{
    add_to_cart, create_cart, get_cart, remove_from_cart, update_cart_discount_codes,
    update_cart_lines, update_cart_note,
};

/// Convert a `CurrencyCode` enum to string.
fn currency_code_to_string<T: std::fmt::Debug>(code: T) -> String {
    format!("{code:?}")
}

/// Convert a `CountryCode` enum to string.
fn country_code_to_string<T: std::fmt::Debug>(code: T) -> String {
    format!("{code:?}")
}

// =============================================================================
// CartData Trait - Generic cart conversion
// =============================================================================

pub fn convert_cart<T: CartData>(cart: T) -> Cart {
    cart.into_cart()
}

pub trait CartData {
    fn into_cart(self) -> Cart;
}

// =============================================================================
// CreateCart Implementation
// =============================================================================

impl CartData for create_cart::CreateCartCartCreateCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_create(self.buyer_identity)),
            cost: convert_cart_cost_create(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_create(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_create(b: create_cart::CartBuyerIdentityFields) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_create(cost: create_cart::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_create(node: create_cart::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        create_cart::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_create(line))
        }
        create_cart::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_create(line: create_cart::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_create(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_create(
    merchandise: create_cart::CartLineFieldsMerchandise,
) -> CartMerchandise {
    // ProductVariant is the only variant in this enum
    let create_cart::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_create(v)
}

fn convert_merchandise_fields_create(v: create_cart::CartMerchandiseFields) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// GetCart Implementation
// =============================================================================

impl CartData for get_cart::GetCartCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_get(self.buyer_identity)),
            cost: convert_cart_cost_get(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_get(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_get(b: get_cart::CartBuyerIdentityFields) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_get(cost: get_cart::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_get(node: get_cart::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        get_cart::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_get(line))
        }
        get_cart::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_get(line: get_cart::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_get(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_get(merchandise: get_cart::CartLineFieldsMerchandise) -> CartMerchandise {
    let get_cart::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_get(v)
}

fn convert_merchandise_fields_get(v: get_cart::CartMerchandiseFields) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// AddToCart Implementation
// =============================================================================

impl CartData for add_to_cart::AddToCartCartLinesAddCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_add(self.buyer_identity)),
            cost: convert_cart_cost_add(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_add(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_add(b: add_to_cart::CartBuyerIdentityFields) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_add(cost: add_to_cart::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_add(node: add_to_cart::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        add_to_cart::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_add(line))
        }
        add_to_cart::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_add(line: add_to_cart::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_add(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_add(merchandise: add_to_cart::CartLineFieldsMerchandise) -> CartMerchandise {
    let add_to_cart::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_add(v)
}

fn convert_merchandise_fields_add(v: add_to_cart::CartMerchandiseFields) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// UpdateCartLines Implementation
// =============================================================================

impl CartData for update_cart_lines::UpdateCartLinesCartLinesUpdateCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_update(self.buyer_identity)),
            cost: convert_cart_cost_update(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_update(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_update(
    b: update_cart_lines::CartBuyerIdentityFields,
) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_update(cost: update_cart_lines::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_update(node: update_cart_lines::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        update_cart_lines::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_update(line))
        }
        update_cart_lines::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_update(line: update_cart_lines::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_update(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_update(
    merchandise: update_cart_lines::CartLineFieldsMerchandise,
) -> CartMerchandise {
    let update_cart_lines::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_update(v)
}

fn convert_merchandise_fields_update(
    v: update_cart_lines::CartMerchandiseFields,
) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// RemoveFromCart Implementation
// =============================================================================

impl CartData for remove_from_cart::RemoveFromCartCartLinesRemoveCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_remove(self.buyer_identity)),
            cost: convert_cart_cost_remove(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_remove(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_remove(
    b: remove_from_cart::CartBuyerIdentityFields,
) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_remove(cost: remove_from_cart::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_remove(node: remove_from_cart::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        remove_from_cart::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_remove(line))
        }
        remove_from_cart::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_remove(line: remove_from_cart::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_remove(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_remove(
    merchandise: remove_from_cart::CartLineFieldsMerchandise,
) -> CartMerchandise {
    let remove_from_cart::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_remove(v)
}

fn convert_merchandise_fields_remove(
    v: remove_from_cart::CartMerchandiseFields,
) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// UpdateCartDiscountCodes Implementation
// =============================================================================

impl CartData for update_cart_discount_codes::UpdateCartDiscountCodesCartDiscountCodesUpdateCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_discount(self.buyer_identity)),
            cost: convert_cart_cost_discount(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_discount(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_discount(
    b: update_cart_discount_codes::CartBuyerIdentityFields,
) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_discount(cost: update_cart_discount_codes::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_discount(
    node: update_cart_discount_codes::CartFieldsLinesEdgesNode,
) -> Option<CartLine> {
    match node {
        update_cart_discount_codes::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_discount(line))
        }
        update_cart_discount_codes::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_discount(line: update_cart_discount_codes::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_discount(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_discount(
    merchandise: update_cart_discount_codes::CartLineFieldsMerchandise,
) -> CartMerchandise {
    let update_cart_discount_codes::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_discount(v)
}

fn convert_merchandise_fields_discount(
    v: update_cart_discount_codes::CartMerchandiseFields,
) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// UpdateCartNote Implementation
// =============================================================================

impl CartData for update_cart_note::UpdateCartNoteCartNoteUpdateCart {
    fn into_cart(self) -> Cart {
        Cart {
            id: self.id,
            checkout_url: self.checkout_url,
            created_at: self.created_at,
            updated_at: self.updated_at,
            note: self.note,
            total_quantity: self.total_quantity,
            attributes: self
                .attributes
                .into_iter()
                .map(|a| Attribute {
                    key: a.key,
                    value: a.value,
                })
                .collect(),
            buyer_identity: Some(convert_buyer_identity_note(self.buyer_identity)),
            cost: convert_cart_cost_note(self.cost),
            discount_codes: self
                .discount_codes
                .into_iter()
                .map(|d| CartDiscountCode {
                    code: d.code,
                    applicable: d.applicable,
                })
                .collect(),
            lines: self
                .lines
                .edges
                .into_iter()
                .filter_map(|e| convert_cart_line_note(e.node))
                .collect(),
        }
    }
}

fn convert_buyer_identity_note(b: update_cart_note::CartBuyerIdentityFields) -> CartBuyerIdentity {
    CartBuyerIdentity {
        email: b.email,
        phone: b.phone,
        country_code: b.country_code.map(country_code_to_string),
        customer: b.customer.map(|c| CartCustomer {
            id: c.id,
            email: c.email,
            first_name: c.first_name,
            last_name: c.last_name,
        }),
    }
}

#[allow(deprecated)]
fn convert_cart_cost_note(cost: update_cart_note::CartCostFields) -> CartCost {
    CartCost {
        subtotal: Money {
            amount: cost.subtotal_amount.amount,
            currency_code: currency_code_to_string(cost.subtotal_amount.currency_code),
        },
        total: Money {
            amount: cost.total_amount.amount,
            currency_code: currency_code_to_string(cost.total_amount.currency_code),
        },
        total_tax: cost.total_tax_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
        total_duty: cost.total_duty_amount.map(|t| Money {
            amount: t.amount,
            currency_code: currency_code_to_string(t.currency_code),
        }),
    }
}

fn convert_cart_line_note(node: update_cart_note::CartFieldsLinesEdgesNode) -> Option<CartLine> {
    match node {
        update_cart_note::CartFieldsLinesEdgesNode::CartLine(line) => {
            Some(convert_cart_line_fields_note(line))
        }
        update_cart_note::CartFieldsLinesEdgesNode::ComponentizableCartLine => {
            warn!("ComponentizableCartLine not yet supported");
            None
        }
    }
}

fn convert_cart_line_fields_note(line: update_cart_note::CartLineFields) -> CartLine {
    CartLine {
        id: line.id,
        quantity: line.quantity,
        attributes: line
            .attributes
            .into_iter()
            .map(|a| Attribute {
                key: a.key,
                value: a.value,
            })
            .collect(),
        cost: CartLineCost {
            amount_per_quantity: Money {
                amount: line.cost.amount_per_quantity.amount,
                currency_code: currency_code_to_string(line.cost.amount_per_quantity.currency_code),
            },
            compare_at_amount_per_quantity: line.cost.compare_at_amount_per_quantity.map(|c| {
                Money {
                    amount: c.amount,
                    currency_code: currency_code_to_string(c.currency_code),
                }
            }),
            subtotal_amount: Money {
                amount: line.cost.subtotal_amount.amount,
                currency_code: currency_code_to_string(line.cost.subtotal_amount.currency_code),
            },
            total_amount: Money {
                amount: line.cost.total_amount.amount,
                currency_code: currency_code_to_string(line.cost.total_amount.currency_code),
            },
        },
        merchandise: convert_merchandise_note(line.merchandise),
        discount_allocations: line
            .discount_allocations
            .into_iter()
            .map(|d| DiscountAllocation {
                discounted_amount: Money {
                    amount: d.discounted_amount.amount,
                    currency_code: currency_code_to_string(d.discounted_amount.currency_code),
                },
            })
            .collect(),
    }
}

fn convert_merchandise_note(
    merchandise: update_cart_note::CartLineFieldsMerchandise,
) -> CartMerchandise {
    let update_cart_note::CartLineFieldsMerchandise::ProductVariant(v) = merchandise;
    convert_merchandise_fields_note(v)
}

fn convert_merchandise_fields_note(v: update_cart_note::CartMerchandiseFields) -> CartMerchandise {
    CartMerchandise {
        id: v.id,
        title: v.title,
        sku: v.sku,
        available_for_sale: v.available_for_sale,
        requires_shipping: v.requires_shipping,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
        product: CartMerchandiseProduct {
            id: v.product.id,
            handle: v.product.handle,
            title: v.product.title,
            vendor: v.product.vendor,
            featured_image: v.product.featured_image.map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        },
    }
}

// =============================================================================
// User Error Conversions
// =============================================================================

pub fn convert_user_error(error: create_cart::CartUserErrorFields) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}

pub fn convert_add_user_error(error: add_to_cart::CartUserErrorFields) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}

pub fn convert_update_user_error(error: update_cart_lines::CartUserErrorFields) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}

pub fn convert_remove_user_error(error: remove_from_cart::CartUserErrorFields) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}

pub fn convert_discount_user_error(
    error: update_cart_discount_codes::CartUserErrorFields,
) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}

pub fn convert_note_user_error(error: update_cart_note::CartUserErrorFields) -> CartUserError {
    CartUserError {
        code: error.code.map(|c| format!("{c:?}")),
        field: error.field,
        message: error.message,
    }
}
