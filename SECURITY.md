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

If a report is declined on technical merit, we will explain why and you are welcome to follow up with additional context. Reports closed as AI-generated are final — see below.

## AI-Generated Reports

We will immediately close vulnerability reports that are AI-generated or AI-assisted without meaningful human verification. This includes reports from LLMs, automated scanning tools fed through AI summarizers, or any submission where the reporter cannot demonstrate a genuine understanding of the alleged vulnerability.

AI-generated security reports [waste maintainer time](https://daniel.haxx.se/blog/2025/07/14/death-by-a-thousand-slops/), produce plausible-sounding but technically incorrect findings, and have [driven established projects to shut down their bug bounty programs entirely](https://daniel.haxx.se/blog/2026/01/26/the-end-of-the-curl-bug-bounty/). Every slop report takes time away from real security work.

A good vulnerability report includes:

- A clear description of the issue written in your own words
- Steps to reproduce the vulnerability
- Evidence of actual impact (not speculative "what if" scenarios)
- Understanding of the code and context involved

If you used an AI tool to help identify a potential issue, you are still expected to verify it yourself, understand it fully, and write the report in your own words. "ChatGPT told me this is vulnerable" is not a report.

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
