/**
 * Multi-platform analytics tracking for Naked Pineapple
 *
 * Supports:
 * - Google Analytics 4 (GA4)
 * - Meta (Facebook/Instagram) Pixel
 * - Google Ads Conversion Tracking (YouTube)
 * - TikTok Pixel
 * - Pinterest Tag
 * - Snapchat Pixel
 * - Microsoft/Bing UET
 * - Twitter/X Pixel
 * - Mixpanel Product Analytics
 * - Crazy Egg (heatmaps, session recordings, A/B testing)
 *
 * Handles HTMX-aware page view tracking and e-commerce events.
 * Only platforms with configured IDs will have code executed.
 */

(function() {
    'use strict';

    // Get tracking IDs from data attributes on body
    var ga4Id = document.body.dataset.ga4Id;
    var metaPixelId = document.body.dataset.metaPixelId;
    var googleAdsId = document.body.dataset.googleAdsId;
    var googleAdsConversionLabel = document.body.dataset.googleAdsConversionLabel;
    var tiktokPixelId = document.body.dataset.tiktokPixelId;
    var pinterestTagId = document.body.dataset.pinterestTagId;
    var snapchatPixelId = document.body.dataset.snapchatPixelId;
    var microsoftUetId = document.body.dataset.microsoftUetId;
    var twitterPixelId = document.body.dataset.twitterPixelId;
    var mixpanelToken = document.body.dataset.mixpanelToken;
    var mixpanelUserId = document.body.dataset.mixpanelUserId;
    var mixpanelUserEmail = document.body.dataset.mixpanelUserEmail;
    var crazyEggId = document.body.dataset.crazyEggId;

    // Check if any tracking is enabled
    var hasAnyTracking = ga4Id || metaPixelId || googleAdsId || tiktokPixelId ||
                         pinterestTagId || snapchatPixelId || microsoftUetId || twitterPixelId ||
                         mixpanelToken;

    if (!hasAnyTracking) {
        // No tracking configured - export no-op functions
        window.NP_Analytics = {
            trackPageView: function() {},
            trackViewItem: function() {},
            trackAddToCart: function() {},
            trackViewCart: function() {},
            trackBeginCheckout: function() {},
            trackPurchase: function() {}
        };
        return;
    }

    // =========================================================================
    // Google Analytics 4 / Google Ads (gtag.js)
    // =========================================================================

    function gtag() {
        window.dataLayer = window.dataLayer || [];
        window.dataLayer.push(arguments);
    }
    window.gtag = gtag;

    // Initialize gtag
    gtag('js', new Date());

    if (ga4Id) {
        gtag('config', ga4Id, { send_page_view: true });
    }

    if (googleAdsId) {
        gtag('config', googleAdsId);
    }

    // =========================================================================
    // Meta (Facebook/Instagram) Pixel
    // =========================================================================

    function fbq() {
        if (window.fbq) {
            window.fbq.apply(window, arguments);
        }
    }

    // =========================================================================
    // TikTok Pixel
    // =========================================================================

    function ttq() {
        if (window.ttq) {
            window.ttq.apply(window, arguments);
        }
    }

    // =========================================================================
    // Pinterest Tag
    // =========================================================================

    function pintrk() {
        if (window.pintrk) {
            window.pintrk.apply(window, arguments);
        }
    }

    // =========================================================================
    // Snapchat Pixel
    // =========================================================================

    function snaptr() {
        if (window.snaptr) {
            window.snaptr.apply(window, arguments);
        }
    }

    // =========================================================================
    // Microsoft/Bing UET
    // =========================================================================

    function uetq() {
        window.uetq = window.uetq || [];
        window.uetq.push.apply(window.uetq, arguments);
    }

    // =========================================================================
    // Twitter/X Pixel
    // =========================================================================

    function twq() {
        if (window.twq) {
            window.twq.apply(window, arguments);
        }
    }

    // =========================================================================
    // Mixpanel Product Analytics
    // =========================================================================

    // Handle user identification and aliasing for Mixpanel
    // Alias links anonymous device ID to user ID on first login
    if (mixpanelToken && window.mixpanel && mixpanelUserId) {
        var aliasKey = 'np_mixpanel_aliased_' + mixpanelUserId;
        var hasAliased = localStorage.getItem(aliasKey);

        if (!hasAliased) {
            // First time seeing this user - alias anonymous profile to user ID
            mixpanel.alias(mixpanelUserId);
            localStorage.setItem(aliasKey, 'true');
        }

        // Always identify the user
        mixpanel.identify(mixpanelUserId);
        if (mixpanelUserEmail) {
            mixpanel.people.set({ '$email': mixpanelUserEmail });
        }
    }

    // =========================================================================
    // Crazy Egg (Heatmaps, Session Recordings, A/B Testing)
    // =========================================================================

    // Set custom variables for user segmentation in Crazy Egg
    // CE2.set() allows filtering heatmaps/recordings by user attributes
    if (crazyEggId && window.CE2) {
        // Slot 1: User login state - useful for comparing logged-in vs anonymous behavior
        var isLoggedIn = mixpanelUserId ? 'yes' : 'no';
        CE2.set(1, 'logged_in', isLoggedIn);
    }

    // =========================================================================
    // Unified Tracking Functions
    // =========================================================================

    /**
     * Track a page view
     * Called on initial load and after HTMX navigation
     */
    function trackPageView(path, title) {
        var pagePath = path || window.location.pathname;
        var pageTitle = title || document.title;

        // GA4
        if (ga4Id) {
            gtag('event', 'page_view', {
                page_path: pagePath,
                page_title: pageTitle,
                page_location: window.location.href
            });
        }

        // Meta Pixel
        if (metaPixelId && window.fbq) {
            fbq('track', 'PageView');
        }

        // TikTok Pixel
        if (tiktokPixelId && window.ttq) {
            ttq.page();
        }

        // Pinterest Tag
        if (pinterestTagId && window.pintrk) {
            pintrk('page');
        }

        // Snapchat Pixel
        if (snapchatPixelId && window.snaptr) {
            snaptr('track', 'PAGE_VIEW');
        }

        // Microsoft UET
        if (microsoftUetId) {
            uetq('event', 'page_view', {});
        }

        // Twitter/X Pixel
        if (twitterPixelId && window.twq) {
            twq('track', 'PageView');
        }

        // Mixpanel
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Page Viewed', {
                'Page Path': pagePath,
                'Page Title': pageTitle
            });
        }
    }

    /**
     * Track view_item event (product page view)
     * @param {Object} item - Product data
     */
    function trackViewItem(item) {
        var value = parseFloat(item.price) || 0;
        var itemId = String(item.id);
        var itemName = item.name || '';
        var category = item.category || '';

        // GA4
        if (ga4Id) {
            gtag('event', 'view_item', {
                currency: 'USD',
                value: value,
                items: [{
                    item_id: itemId,
                    item_name: itemName,
                    price: value,
                    item_category: category,
                    item_variant: item.variant || '',
                    quantity: 1
                }]
            });
        }

        // Meta Pixel - ViewContent
        if (metaPixelId) {
            fbq('track', 'ViewContent', {
                content_ids: [itemId],
                content_name: itemName,
                content_type: 'product',
                content_category: category,
                value: value,
                currency: 'USD'
            });
        }

        // TikTok Pixel - ViewContent
        if (tiktokPixelId) {
            ttq.track('ViewContent', {
                content_id: itemId,
                content_name: itemName,
                content_type: 'product',
                content_category: category,
                value: value,
                currency: 'USD'
            });
        }

        // Pinterest Tag - PageVisit
        if (pinterestTagId) {
            pintrk('track', 'pagevisit', {
                product_id: itemId,
                product_name: itemName,
                product_category: category,
                value: value,
                currency: 'USD'
            });
        }

        // Snapchat Pixel - VIEW_CONTENT
        if (snapchatPixelId) {
            snaptr('track', 'VIEW_CONTENT', {
                item_ids: [itemId],
                price: value,
                currency: 'USD'
            });
        }

        // Microsoft UET - product view
        if (microsoftUetId) {
            uetq('event', 'view_item', {
                ecomm_prodid: itemId,
                ecomm_pagetype: 'product',
                revenue_value: value,
                currency: 'USD'
            });
        }

        // Twitter/X Pixel - ViewContent
        if (twitterPixelId) {
            twq('track', 'ViewContent', {
                content_ids: [itemId],
                content_name: itemName,
                value: value,
                currency: 'USD'
            });
        }

        // Mixpanel - Product Viewed
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Product Viewed', {
                'Product ID': itemId,
                'Product Name': itemName,
                'Price': value,
                'Category': category
            });
        }
    }

    /**
     * Track add_to_cart event
     * @param {Object} item - Product data
     */
    function trackAddToCart(item) {
        var quantity = parseInt(item.quantity) || 1;
        var price = parseFloat(item.price) || 0;
        var value = price * quantity;
        var itemId = String(item.id);
        var itemName = item.name || '';
        var category = item.category || '';

        // GA4
        if (ga4Id) {
            gtag('event', 'add_to_cart', {
                currency: 'USD',
                value: value,
                items: [{
                    item_id: itemId,
                    item_name: itemName,
                    price: price,
                    item_category: category,
                    item_variant: item.variant || '',
                    quantity: quantity
                }]
            });
        }

        // Meta Pixel - AddToCart
        if (metaPixelId) {
            fbq('track', 'AddToCart', {
                content_ids: [itemId],
                content_name: itemName,
                content_type: 'product',
                value: value,
                currency: 'USD'
            });
        }

        // TikTok Pixel - AddToCart
        if (tiktokPixelId) {
            ttq.track('AddToCart', {
                content_id: itemId,
                content_name: itemName,
                content_type: 'product',
                quantity: quantity,
                value: value,
                currency: 'USD'
            });
        }

        // Pinterest Tag - AddToCart
        if (pinterestTagId) {
            pintrk('track', 'addtocart', {
                product_id: itemId,
                product_name: itemName,
                product_quantity: quantity,
                value: value,
                currency: 'USD'
            });
        }

        // Snapchat Pixel - ADD_CART
        if (snapchatPixelId) {
            snaptr('track', 'ADD_CART', {
                item_ids: [itemId],
                price: value,
                currency: 'USD',
                number_items: quantity
            });
        }

        // Microsoft UET - add_to_cart
        if (microsoftUetId) {
            uetq('event', 'add_to_cart', {
                ecomm_prodid: itemId,
                ecomm_pagetype: 'cart',
                revenue_value: value,
                currency: 'USD'
            });
        }

        // Twitter/X Pixel - AddToCart
        if (twitterPixelId) {
            twq('track', 'AddToCart', {
                content_ids: [itemId],
                content_name: itemName,
                value: value,
                currency: 'USD'
            });
        }

        // Mixpanel - Product Added to Cart
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Product Added to Cart', {
                'Product ID': itemId,
                'Product Name': itemName,
                'Price': price,
                'Quantity': quantity,
                'Cart Value': value,
                'Category': category
            });
        }
    }

    /**
     * Track view_cart event
     * @param {Object} cart - Cart data
     */
    function trackViewCart(cart) {
        var value = parseFloat(cart.value) || 0;
        var contentIds = cart.items.map(function(item) { return String(item.id); });
        var numItems = cart.items.length;

        // GA4
        if (ga4Id) {
            var ga4Items = cart.items.map(function(item) {
                return {
                    item_id: String(item.id),
                    item_name: item.name,
                    price: parseFloat(item.price) || 0,
                    quantity: parseInt(item.quantity) || 1,
                    item_variant: item.variant || ''
                };
            });

            gtag('event', 'view_cart', {
                currency: 'USD',
                value: value,
                items: ga4Items
            });
        }

        // Meta Pixel - custom ViewCart event
        if (metaPixelId) {
            fbq('trackCustom', 'ViewCart', {
                content_ids: contentIds,
                value: value,
                currency: 'USD',
                num_items: numItems
            });
        }

        // TikTok doesn't have a standard view_cart event

        // Pinterest doesn't have a standard view_cart event

        // Snapchat doesn't have a standard view_cart event

        // Microsoft UET
        if (microsoftUetId) {
            uetq('event', 'view_cart', {
                ecomm_prodid: contentIds,
                ecomm_pagetype: 'cart',
                revenue_value: value,
                currency: 'USD'
            });
        }

        // Twitter doesn't have a standard view_cart event

        // Mixpanel - Cart Viewed
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Cart Viewed', {
                'Cart Value': value,
                'Item Count': numItems
            });
        }
    }

    /**
     * Track begin_checkout event
     * @param {Object} cart - Cart data
     */
    function trackBeginCheckout(cart) {
        var value = parseFloat(cart.value) || 0;
        var contentIds = cart.items.map(function(item) { return String(item.id); });
        var numItems = cart.items.length;

        // GA4
        if (ga4Id) {
            var ga4Items = cart.items.map(function(item) {
                return {
                    item_id: String(item.id),
                    item_name: item.name,
                    price: parseFloat(item.price) || 0,
                    quantity: parseInt(item.quantity) || 1,
                    item_variant: item.variant || ''
                };
            });

            gtag('event', 'begin_checkout', {
                currency: 'USD',
                value: value,
                items: ga4Items
            });
        }

        // Meta Pixel - InitiateCheckout
        if (metaPixelId) {
            fbq('track', 'InitiateCheckout', {
                content_ids: contentIds,
                value: value,
                currency: 'USD',
                num_items: numItems
            });
        }

        // TikTok Pixel - InitiateCheckout
        if (tiktokPixelId) {
            ttq.track('InitiateCheckout', {
                content_ids: contentIds,
                value: value,
                currency: 'USD'
            });
        }

        // Pinterest Tag - Checkout
        if (pinterestTagId) {
            pintrk('track', 'checkout', {
                product_id: contentIds,
                value: value,
                order_quantity: numItems,
                currency: 'USD'
            });
        }

        // Snapchat Pixel - START_CHECKOUT
        if (snapchatPixelId) {
            snaptr('track', 'START_CHECKOUT', {
                item_ids: contentIds,
                price: value,
                currency: 'USD',
                number_items: numItems
            });
        }

        // Microsoft UET - begin_checkout
        if (microsoftUetId) {
            uetq('event', 'begin_checkout', {
                ecomm_prodid: contentIds,
                ecomm_pagetype: 'checkout',
                revenue_value: value,
                currency: 'USD'
            });
        }

        // Twitter/X Pixel - InitiateCheckout
        if (twitterPixelId) {
            twq('track', 'InitiateCheckout', {
                content_ids: contentIds,
                value: value,
                currency: 'USD'
            });
        }

        // Mixpanel - Checkout Started
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Checkout Started', {
                'Cart Value': value,
                'Item Count': numItems
            });
        }
    }

    /**
     * Track purchase event
     * @param {Object} order - Order data
     */
    function trackPurchase(order) {
        var value = parseFloat(order.value) || 0;
        var shipping = parseFloat(order.shipping) || 0;
        var tax = parseFloat(order.tax) || 0;
        var transactionId = String(order.transaction_id);
        var contentIds = order.items.map(function(item) { return String(item.id); });
        var numItems = order.items.length;

        // GA4
        if (ga4Id) {
            var ga4Items = order.items.map(function(item) {
                return {
                    item_id: String(item.id),
                    item_name: item.name,
                    price: parseFloat(item.price) || 0,
                    quantity: parseInt(item.quantity) || 1,
                    item_variant: item.variant || ''
                };
            });

            gtag('event', 'purchase', {
                transaction_id: transactionId,
                currency: 'USD',
                value: value,
                shipping: shipping,
                tax: tax,
                items: ga4Items
            });
        }

        // Google Ads Conversion
        if (googleAdsId && googleAdsConversionLabel) {
            gtag('event', 'conversion', {
                send_to: googleAdsId + '/' + googleAdsConversionLabel,
                value: value,
                currency: 'USD',
                transaction_id: transactionId
            });
        }

        // Meta Pixel - Purchase
        if (metaPixelId) {
            fbq('track', 'Purchase', {
                content_ids: contentIds,
                content_type: 'product',
                value: value,
                currency: 'USD',
                num_items: numItems
            });
        }

        // TikTok Pixel - CompletePayment
        if (tiktokPixelId) {
            ttq.track('CompletePayment', {
                content_ids: contentIds,
                content_type: 'product',
                value: value,
                currency: 'USD'
            });
        }

        // Pinterest Tag - Checkout (purchase)
        if (pinterestTagId) {
            pintrk('track', 'checkout', {
                product_id: contentIds,
                value: value,
                order_quantity: numItems,
                order_id: transactionId,
                currency: 'USD'
            });
        }

        // Snapchat Pixel - PURCHASE
        if (snapchatPixelId) {
            snaptr('track', 'PURCHASE', {
                item_ids: contentIds,
                price: value,
                currency: 'USD',
                transaction_id: transactionId,
                number_items: numItems
            });
        }

        // Microsoft UET - purchase
        if (microsoftUetId) {
            uetq('event', 'purchase', {
                ecomm_prodid: contentIds,
                ecomm_pagetype: 'purchase',
                revenue_value: value,
                currency: 'USD',
                transaction_id: transactionId
            });
        }

        // Twitter/X Pixel - Purchase
        if (twitterPixelId) {
            twq('track', 'Purchase', {
                content_ids: contentIds,
                value: value,
                currency: 'USD',
                num_items: numItems
            });
        }

        // Mixpanel - Order Completed + Lifetime Value tracking
        if (mixpanelToken && window.mixpanel) {
            mixpanel.track('Order Completed', {
                'Order ID': transactionId,
                'Revenue': value,
                'Shipping': shipping,
                'Tax': tax,
                'Products': numItems
            });
            // Track lifetime value (revenue per user)
            mixpanel.people.track_charge(value);
        }
    }

    // =========================================================================
    // HTMX Navigation Tracking
    // =========================================================================

    // Listen for HTMX navigation to track page views
    document.body.addEventListener('htmx:afterSettle', function(event) {
        var target = event.detail.target;
        if (target && (target.id === 'MainContent' || target === document.body || target.tagName === 'BODY')) {
            requestAnimationFrame(function() {
                trackPageView();
            });
        }
    });

    // Also listen for htmx:pushedIntoHistory for URL-changing navigations
    document.body.addEventListener('htmx:pushedIntoHistory', function() {
        requestAnimationFrame(function() {
            trackPageView();
        });
    });

    // =========================================================================
    // Global Error Handlers (forward to Sentry if available)
    // =========================================================================

    // Forward uncaught errors to Sentry
    window.onerror = function(message, source, lineno, colno, error) {
        if (window.Sentry && window.Sentry.captureException) {
            window.Sentry.captureException(error || new Error(message), {
                extra: {
                    source: source,
                    lineno: lineno,
                    colno: colno
                }
            });
        }
        // Don't prevent default error handling
        return false;
    };

    // Forward unhandled promise rejections to Sentry
    window.addEventListener('unhandledrejection', function(event) {
        if (window.Sentry && window.Sentry.captureException) {
            var error = event.reason instanceof Error
                ? event.reason
                : new Error('Unhandled promise rejection: ' + String(event.reason));
            window.Sentry.captureException(error);
        }
    });

    // =========================================================================
    // Export Public API
    // =========================================================================

    window.NP_Analytics = {
        trackPageView: trackPageView,
        trackViewItem: trackViewItem,
        trackAddToCart: trackAddToCart,
        trackViewCart: trackViewCart,
        trackBeginCheckout: trackBeginCheckout,
        trackPurchase: trackPurchase
    };

})();
