/**
 * Naked Pineapple Support Chat Widget
 *
 * Handles: open/close, Turnstile verification, conversation lifecycle,
 * SSE streaming with progressive text rendering, and history loading.
 */
(function () {
    'use strict';

    if (window._chatInitialized) return;
    window._chatInitialized = true;

    // =========================================================================
    // DOM References
    // =========================================================================
    var fab = document.getElementById('chat-fab');
    var panel = document.getElementById('chat-panel');
    if (!fab || !panel) return;

    var fabIcon = document.getElementById('chat-fab-icon');
    var fabCloseIcon = document.getElementById('chat-fab-close-icon');
    var messages = document.getElementById('chat-messages');
    var welcome = document.getElementById('chat-welcome');
    var toolIndicator = document.getElementById('chat-tool-indicator');
    var toolName = document.getElementById('chat-tool-name');
    var form = document.getElementById('chat-form');
    var input = document.getElementById('chat-input');
    var sendBtn = document.getElementById('chat-send');
    var errorEl = document.getElementById('chat-error');

    // =========================================================================
    // State
    // =========================================================================
    var conversationId = null;
    var isOpen = false;
    var isSending = false;
    var turnstileToken = null;
    var turnstileWidgetId = null;
    var turnstileReady = false;
    var siteKey = panel.dataset.turnstileSiteKey || '';

    // Restore state from localStorage
    try {
        var savedConvId = localStorage.getItem('np_chat_conv_id');
        if (savedConvId) conversationId = parseInt(savedConvId, 10) || null;
        if (localStorage.getItem('np_chat_open') === 'true') {
            requestAnimationFrame(function () { openChat(); });
        }
    } catch (e) { /* localStorage unavailable */ }

    // =========================================================================
    // Turnstile
    // =========================================================================
    function initTurnstile() {
        if (turnstileWidgetId != null || !siteKey) return;
        if (typeof window.turnstile === 'undefined') return;

        var container = document.getElementById('chat-turnstile');
        if (!container) return;

        turnstileWidgetId = window.turnstile.render(container, {
            sitekey: siteKey,
            callback: function (token) {
                turnstileToken = token;
                turnstileReady = true;
            },
            'expired-callback': function () {
                turnstileToken = null;
                turnstileReady = false;
            },
            'error-callback': function () {
                turnstileToken = null;
                turnstileReady = false;
            },
            size: 'invisible',
        });
    }

    function ensureTurnstileToken() {
        return new Promise(function (resolve, reject) {
            if (turnstileToken) {
                resolve(turnstileToken);
                return;
            }
            if (typeof window.turnstile === 'undefined') {
                reject(new Error('Turnstile not loaded'));
                return;
            }
            // Reset to get a fresh token
            if (turnstileWidgetId != null) {
                window.turnstile.reset(turnstileWidgetId);
            } else {
                initTurnstile();
            }
            // Poll for token (invisible challenge resolves quickly)
            var attempts = 0;
            var interval = setInterval(function () {
                attempts++;
                if (turnstileToken) {
                    clearInterval(interval);
                    resolve(turnstileToken);
                } else if (attempts > 50) {
                    clearInterval(interval);
                    reject(new Error('Turnstile verification timed out'));
                }
            }, 100);
        });
    }

    // Load Turnstile script on first open
    function loadTurnstileScript() {
        if (document.getElementById('turnstile-script') || !siteKey) return;
        var script = document.createElement('script');
        script.id = 'turnstile-script';
        script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?onload=onTurnstileLoad';
        script.async = true;
        document.head.appendChild(script);
        window.onTurnstileLoad = function () {
            initTurnstile();
        };
    }

    // =========================================================================
    // Open / Close
    // =========================================================================
    function openChat() {
        isOpen = true;
        panel.classList.remove('hidden');
        // Trigger reflow before adding visible classes
        panel.offsetHeight;
        panel.classList.remove('opacity-0', 'scale-95', 'pointer-events-none');
        panel.classList.add('opacity-100', 'scale-100', 'pointer-events-auto');
        panel.style.display = 'flex';

        // Swap FAB icon
        fabIcon.classList.add('hidden');
        fabCloseIcon.classList.remove('hidden');

        loadTurnstileScript();
        input.focus();

        // Load history if resuming a conversation
        if (conversationId && !messages.querySelector('.chat-msg')) {
            loadHistory();
        }

        try { localStorage.setItem('np_chat_open', 'true'); } catch (e) {}

        window.trapFocus(panel);
    }

    function closeChat() {
        isOpen = false;
        window.releaseFocus(panel);

        panel.classList.add('opacity-0', 'scale-95', 'pointer-events-none');
        panel.classList.remove('opacity-100', 'scale-100', 'pointer-events-auto');

        // Swap FAB icon back
        fabIcon.classList.remove('hidden');
        fabCloseIcon.classList.add('hidden');

        setTimeout(function () {
            if (!isOpen) {
                panel.classList.add('hidden');
                panel.style.display = '';
            }
        }, 200);

        try { localStorage.setItem('np_chat_open', 'false'); } catch (e) {}
    }

    window.toggleChat = function () {
        if (isOpen) closeChat(); else openChat();
    };
    window.closeChat = closeChat;

    // =========================================================================
    // Message Rendering
    // =========================================================================
    function scrollToBottom() {
        messages.scrollTop = messages.scrollHeight;
    }

    function hideWelcome() {
        if (welcome) welcome.classList.add('hidden');
    }

    function appendMessage(role, text) {
        hideWelcome();
        var wrapper = document.createElement('div');
        wrapper.className = 'chat-msg flex ' + (role === 'customer' ? 'justify-end' : 'justify-start');

        var bubble = document.createElement('div');
        bubble.className = role === 'customer'
            ? 'max-w-[85%] rounded-2xl rounded-br-md px-4 py-2.5 text-sm bg-primary text-primary-foreground'
            : 'max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2.5 text-sm bg-muted text-foreground';

        if (role === 'assistant') {
            bubble.classList.add('chat-assistant-msg');
            bubble.innerHTML = formatMarkdown(text);
        } else {
            bubble.textContent = text;
        }

        wrapper.appendChild(bubble);
        messages.appendChild(wrapper);
        scrollToBottom();
        return bubble;
    }

    function createStreamingBubble() {
        hideWelcome();
        var wrapper = document.createElement('div');
        wrapper.className = 'chat-msg flex justify-start';
        var bubble = document.createElement('div');
        bubble.className = 'chat-assistant-msg max-w-[85%] rounded-2xl rounded-bl-md px-4 py-2.5 text-sm bg-muted text-foreground';
        wrapper.appendChild(bubble);
        messages.appendChild(wrapper);
        scrollToBottom();
        return bubble;
    }

    function formatMarkdown(text) {
        // Minimal markdown: **bold**, *italic*, `code`, newlines
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
            .replace(/\*(.+?)\*/g, '<em>$1</em>')
            .replace(/`(.+?)`/g, '<code class="bg-background/50 px-1 rounded text-xs">$1</code>')
            .replace(/\n/g, '<br>');
    }

    // =========================================================================
    // Error Display
    // =========================================================================
    var pendingRetry = null;

    function showError(msg, retryFn) {
        errorEl.innerHTML = '';
        var span = document.createElement('span');
        span.textContent = msg;
        errorEl.appendChild(span);

        if (retryFn) {
            pendingRetry = retryFn;
            var btn = document.createElement('button');
            btn.type = 'button';
            btn.className = 'underline font-medium ml-1 hover:text-primary transition-colors';
            btn.textContent = 'Try again';
            btn.addEventListener('click', function () {
                clearError();
                pendingRetry = null;
                retryFn();
            });
            errorEl.appendChild(btn);
        } else {
            pendingRetry = null;
            setTimeout(function () { errorEl.classList.add('hidden'); }, 5000);
        }

        errorEl.classList.remove('hidden');
    }

    function clearError() {
        errorEl.classList.add('hidden');
        errorEl.innerHTML = '';
        pendingRetry = null;
    }

    // =========================================================================
    // Conversation Lifecycle
    // =========================================================================
    function startConversation() {
        return ensureTurnstileToken().then(function (token) {
            return fetch('/support/chat', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ 'cf-turnstile-response': token }),
            });
        }).then(function (res) {
            if (!res.ok) {
                return res.json().then(function (body) {
                    throw new Error(body.error || 'Failed to start chat');
                });
            }
            return res.json();
        }).then(function (data) {
            conversationId = data.id;
            try { localStorage.setItem('np_chat_conv_id', String(data.id)); } catch (e) {}

            // Reset turnstile token (single-use)
            turnstileToken = null;
            if (turnstileWidgetId != null && typeof window.turnstile !== 'undefined') {
                window.turnstile.reset(turnstileWidgetId);
            }

            // Load history if resuming
            if (!data.is_new) {
                loadHistory();
            }

            return data;
        });
    }

    function loadHistory() {
        if (!conversationId) return;
        fetch('/support/chat/' + conversationId + '/messages')
            .then(function (res) {
                if (!res.ok) {
                    // Conversation may have been deleted/expired
                    if (res.status === 404 || res.status === 403) {
                        conversationId = null;
                        try { localStorage.removeItem('np_chat_conv_id'); } catch (e) {}
                        return [];
                    }
                    throw new Error('Failed to load messages');
                }
                return res.json();
            })
            .then(function (msgs) {
                if (!msgs || msgs.length === 0) return;
                hideWelcome();
                // Clear existing messages (avoid duplicates)
                var existing = messages.querySelectorAll('.chat-msg');
                existing.forEach(function (el) { el.remove(); });

                msgs.forEach(function (msg) {
                    if (msg.role === 'customer') {
                        var content = typeof msg.content === 'string'
                            ? msg.content
                            : (msg.content.text || JSON.stringify(msg.content));
                        appendMessage('customer', content);
                    } else if (msg.role === 'assistant') {
                        var text = typeof msg.content === 'string'
                            ? msg.content
                            : (msg.content.text || '');
                        if (text) appendMessage('assistant', text);
                    }
                    // Skip tool_use, tool_result, system messages in UI
                });
                scrollToBottom();
            })
            .catch(function (err) {
                console.error('Chat history load error:', err);
            });
    }

    // =========================================================================
    // SSE Streaming
    // =========================================================================
    function sendMessage(text) {
        if (isSending || !text.trim()) return;
        isSending = true;
        sendBtn.disabled = true;
        clearError();

        // Show customer message immediately
        appendMessage('customer', text.trim());
        input.value = '';
        autoResizeInput();

        var doSend = function () {
            var streamBubble = createStreamingBubble();
            var streamedText = '';

            fetch('/support/chat/' + conversationId + '/messages/stream', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ message: text.trim() }),
            }).then(function (res) {
                if (!res.ok) {
                    return res.text().then(function (body) {
                        var msg = 'Something went wrong';
                        try { msg = JSON.parse(body).error || msg; } catch (e) {}
                        throw new Error(msg);
                    });
                }
                var reader = res.body.getReader();
                var decoder = new TextDecoder();
                var buffer = '';

                function processChunk() {
                    return reader.read().then(function (result) {
                        if (result.done) {
                            finishStream();
                            return;
                        }
                        buffer += decoder.decode(result.value, { stream: true });
                        var lines = buffer.split('\n');
                        buffer = lines.pop() || '';

                        for (var i = 0; i < lines.length; i++) {
                            var line = lines[i].trim();
                            if (line.startsWith('data:')) {
                                var jsonStr = line.substring(5).trim();
                                if (!jsonStr) continue;
                                try {
                                    handleStreamEvent(JSON.parse(jsonStr), streamBubble);
                                } catch (e) {
                                    console.error('Failed to parse SSE event:', e);
                                }
                            }
                        }
                        return processChunk();
                    });
                }

                function handleStreamEvent(event, bubble) {
                    switch (event.type) {
                        case 'text_delta':
                            streamedText += event.text;
                            bubble.innerHTML = formatMarkdown(streamedText);
                            scrollToBottom();
                            break;
                        case 'tool_use':
                            showToolIndicator(event.name);
                            break;
                        case 'tool_result':
                            hideToolIndicator();
                            break;
                        case 'message_complete':
                            hideToolIndicator();
                            break;
                        case 'error':
                            showError(event.message);
                            break;
                    }
                }

                function finishStream() {
                    hideToolIndicator();
                    isSending = false;
                    updateSendButton();
                    // Remove empty bubbles (if no text was streamed)
                    if (!streamedText && streamBubble.parentNode) {
                        streamBubble.parentNode.remove();
                    }
                }

                return processChunk();
            }).catch(function (err) {
                console.error('Chat stream error:', err);
                // Remove the empty streaming bubble
                if (streamBubble.parentNode) streamBubble.parentNode.remove();
                isSending = false;
                updateSendButton();
                showError(err.message || 'Connection lost.', function () {
                    sendMessage(text);
                });
            });
        };

        // Start conversation if needed, then send
        if (!conversationId) {
            startConversation()
                .then(doSend)
                .catch(function (err) {
                    console.error('Failed to start conversation:', err);
                    // Remove the customer message since it failed
                    var lastMsg = messages.querySelector('.chat-msg:last-child');
                    if (lastMsg) lastMsg.remove();
                    isSending = false;
                    updateSendButton();
                    input.value = text;
                    autoResizeInput();
                    updateSendButton();
                    showError(err.message || 'Failed to connect.', function () {
                        sendMessage(text);
                    });
                });
        } else {
            doSend();
        }
    }

    // =========================================================================
    // Tool Indicator
    // =========================================================================
    var TOOL_LABELS = {
        'lookup_faq': 'Searching our knowledge base...',
        'lookup_product': 'Looking up product info...',
        'lookup_order_status': 'Checking your order...',
        'lookup_subscription': 'Checking your subscription...',
        'request_human_help': 'Connecting you to our team...',
    };

    function showToolIndicator(name) {
        toolName.textContent = TOOL_LABELS[name] || 'Looking that up...';
        toolIndicator.classList.remove('hidden');
        scrollToBottom();
    }

    function hideToolIndicator() {
        toolIndicator.classList.add('hidden');
    }

    // =========================================================================
    // Input Handling
    // =========================================================================
    function autoResizeInput() {
        input.style.height = 'auto';
        input.style.height = Math.min(input.scrollHeight, 120) + 'px';
    }

    function updateSendButton() {
        sendBtn.disabled = isSending || !input.value.trim();
    }

    input.addEventListener('input', function () {
        autoResizeInput();
        updateSendButton();
    });

    input.addEventListener('keydown', function (e) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            if (!isSending && input.value.trim()) {
                sendMessage(input.value);
            }
        }
    });

    form.addEventListener('submit', function (e) {
        e.preventDefault();
        if (!isSending && input.value.trim()) {
            sendMessage(input.value);
        }
    });

    // =========================================================================
    // Event Delegation Integration
    // =========================================================================
    // The base template's event delegation handles data-action="toggle-chat"
    // by calling window.toggleChat(). Escape key closing is also handled there.
})();
