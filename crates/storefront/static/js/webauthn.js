/**
 * WebAuthn Passkey Authentication
 *
 * Client-side JavaScript for passkey registration and authentication.
 */

// Base64URL encoding/decoding utilities
function base64UrlToArrayBuffer(base64url) {
    const base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');
    const padding = '='.repeat((4 - base64.length % 4) % 4);
    const binary = atob(base64 + padding);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
    }
    return bytes.buffer;
}

function arrayBufferToBase64Url(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.length; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    const base64 = btoa(binary);
    return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/**
 * Register a new passkey for the current user.
 *
 * @param {string} name - User-friendly name for the passkey (e.g., "MacBook")
 * @returns {Promise<{success: boolean, credentialId?: number, error?: string}>}
 */
async function registerPasskey(name = 'Passkey') {
    try {
        // Start registration - get challenge from server
        const startResponse = await fetch('/api/auth/webauthn/register/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name }),
        });

        if (!startResponse.ok) {
            const error = await startResponse.json();
            return { success: false, error: error.error || 'Failed to start registration' };
        }

        const { options } = await startResponse.json();

        // Convert base64url fields to ArrayBuffer for WebAuthn API
        const publicKeyOptions = {
            ...options.publicKey,
            challenge: base64UrlToArrayBuffer(options.publicKey.challenge),
            user: {
                ...options.publicKey.user,
                id: base64UrlToArrayBuffer(options.publicKey.user.id),
            },
        };

        if (options.publicKey.excludeCredentials) {
            publicKeyOptions.excludeCredentials = options.publicKey.excludeCredentials.map(cred => ({
                ...cred,
                id: base64UrlToArrayBuffer(cred.id),
            }));
        }

        // Prompt user to create credential
        const credential = await navigator.credentials.create({
            publicKey: publicKeyOptions,
        });

        if (!credential) {
            return { success: false, error: 'User cancelled registration' };
        }

        // Prepare credential for server
        const credentialResponse = {
            id: credential.id,
            rawId: arrayBufferToBase64Url(credential.rawId),
            type: credential.type,
            response: {
                clientDataJSON: arrayBufferToBase64Url(credential.response.clientDataJSON),
                attestationObject: arrayBufferToBase64Url(credential.response.attestationObject),
            },
        };

        if (credential.response.getTransports) {
            credentialResponse.response.transports = credential.response.getTransports();
        }

        // Finish registration - send credential to server
        const finishResponse = await fetch('/api/auth/webauthn/register/finish', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                credential: credentialResponse,
                name,
            }),
        });

        if (!finishResponse.ok) {
            const error = await finishResponse.json();
            return { success: false, error: error.error || 'Failed to finish registration' };
        }

        const result = await finishResponse.json();
        return { success: true, credentialId: result.credential_id };

    } catch (error) {
        console.error('Passkey registration error:', error);

        if (error.name === 'NotAllowedError') {
            return { success: false, error: 'Registration was cancelled or not allowed' };
        }
        if (error.name === 'InvalidStateError') {
            return { success: false, error: 'This passkey is already registered' };
        }
        if (error.name === 'NotSupportedError') {
            return { success: false, error: 'Passkeys are not supported on this device' };
        }

        return { success: false, error: error.message || 'An error occurred during registration' };
    }
}

/**
 * Authenticate with a passkey.
 *
 * @param {string} email - User's email address
 * @returns {Promise<{success: boolean, redirect?: string, error?: string}>}
 */
async function loginWithPasskey(email) {
    try {
        // If no email provided, try to get from form
        if (!email) {
            const emailInput = document.getElementById('passkey-email');
            if (emailInput) {
                email = emailInput.value;
            }
        }

        if (!email) {
            return { success: false, error: 'Please enter your email address' };
        }

        // Start authentication - get challenge from server
        const startResponse = await fetch('/api/auth/webauthn/authenticate/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email }),
        });

        if (!startResponse.ok) {
            const error = await startResponse.json();
            return { success: false, error: error.error || 'Failed to start authentication' };
        }

        const { options } = await startResponse.json();

        // Convert base64url fields to ArrayBuffer for WebAuthn API
        const publicKeyOptions = {
            ...options.publicKey,
            challenge: base64UrlToArrayBuffer(options.publicKey.challenge),
        };

        if (options.publicKey.allowCredentials) {
            publicKeyOptions.allowCredentials = options.publicKey.allowCredentials.map(cred => ({
                ...cred,
                id: base64UrlToArrayBuffer(cred.id),
            }));
        }

        // Prompt user to authenticate
        const credential = await navigator.credentials.get({
            publicKey: publicKeyOptions,
        });

        if (!credential) {
            return { success: false, error: 'User cancelled authentication' };
        }

        // Prepare credential for server
        const credentialResponse = {
            id: credential.id,
            rawId: arrayBufferToBase64Url(credential.rawId),
            type: credential.type,
            response: {
                clientDataJSON: arrayBufferToBase64Url(credential.response.clientDataJSON),
                authenticatorData: arrayBufferToBase64Url(credential.response.authenticatorData),
                signature: arrayBufferToBase64Url(credential.response.signature),
            },
        };

        if (credential.response.userHandle) {
            credentialResponse.response.userHandle = arrayBufferToBase64Url(credential.response.userHandle);
        }

        // Finish authentication - send credential to server
        const finishResponse = await fetch('/api/auth/webauthn/authenticate/finish', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ credential: credentialResponse }),
        });

        if (!finishResponse.ok) {
            const error = await finishResponse.json();
            return { success: false, error: error.error || 'Failed to finish authentication' };
        }

        const result = await finishResponse.json();

        // Redirect to account page on success
        if (result.success && result.redirect) {
            window.location.href = result.redirect;
        }

        return { success: true, redirect: result.redirect };

    } catch (error) {
        console.error('Passkey authentication error:', error);

        if (error.name === 'NotAllowedError') {
            return { success: false, error: 'Authentication was cancelled or not allowed' };
        }
        if (error.name === 'NotSupportedError') {
            return { success: false, error: 'Passkeys are not supported on this device' };
        }

        return { success: false, error: error.message || 'An error occurred during authentication' };
    }
}

/**
 * Check if WebAuthn is supported in the current browser.
 *
 * @returns {boolean}
 */
function isWebAuthnSupported() {
    return window.PublicKeyCredential !== undefined;
}

/**
 * Check if the platform authenticator (e.g., Touch ID, Face ID) is available.
 *
 * @returns {Promise<boolean>}
 */
async function isPlatformAuthenticatorAvailable() {
    if (!isWebAuthnSupported()) {
        return false;
    }
    try {
        return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
    } catch {
        return false;
    }
}

// Export for use in other scripts
window.WebAuthn = {
    registerPasskey,
    loginWithPasskey,
    isWebAuthnSupported,
    isPlatformAuthenticatorAvailable,
};
