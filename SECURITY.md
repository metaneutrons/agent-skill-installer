# Security Policy

## Supported Versions

Security fixes are provided for the latest release. Reports affecting an older
release should state whether the issue also reproduces on the current version.

## Reporting a Vulnerability

Report vulnerabilities privately through
[GitHub Security Advisories](https://github.com/metaneutrons/agent-skill-installer/security/advisories/new).

Please include the affected version, the platform, reproduction steps, the
impact and any known mitigation. Remove personal data and the contents of your
own skill directories before submitting the report.

You should receive an acknowledgement within three business days. Status
updates follow at least every seven days until the report is resolved or
closed. Please do not disclose the issue publicly before a coordinated release.

## Scope

This crate writes into directories that AI coding tools read their skills from.
In scope are, among others: writing outside the intended skill directory
(through symlinks, `..` or crafted names), overwriting or deleting content the
crate did not install, and installing without the consent the calling program
asked for.
