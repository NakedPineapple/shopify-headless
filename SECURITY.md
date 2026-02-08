# Security Policy

## Supported Versions

Only the latest release is supported with security updates.

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |
| < latest | :x:               |

## Reporting a Vulnerability

Please report security vulnerabilities through [GitHub's private vulnerability reporting](https://github.com/NakedPineapple/shopify-headless/security/advisories/new) or by emailing security@pineappleskinco.com. Do not open a public issue.

You should receive an initial response within 72 hours acknowledging your report. From there you can expect:

- **Triage** within 1 week to confirm whether the vulnerability is accepted or declined.
- **A fix or mitigation** for accepted vulnerabilities, typically within 30 days depending on severity and complexity.
- **Credit** in the fix commit and release notes, unless you prefer to remain anonymous.

If a report is declined, we will explain why and you are welcome to follow up with additional context.

## Scope

This policy covers the Naked Pineapple storefront and admin applications, including:

- Server-side Rust code (Axum routes, database queries, Shopify API integrations)
- Client-side JavaScript in templates
- GitHub Actions workflows
- Docker and deployment configurations

Third-party dependencies are monitored via Dependabot and CodeQL.

## Out of Scope

- Shopify's own infrastructure and APIs
- Cloudflare, Fly.io, or other third-party service vulnerabilities (report these to the respective providers)
- Denial of service through volumetric traffic (this is an infrastructure concern, not an application vulnerability)
