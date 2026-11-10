# Requests and environments

## Requests

A request carries a method, a URL, headers, a body and settings such as
timeouts, redirects and certificates. Folder (request group) nesting works
like the original client: folders can contain requests and other folders.

## Environments

Every workspace has a **base environment**, created automatically, and any
number of **sub-environments**. Sub-environments override the base
environment key by key, and nested sub-environments chain the same way the
original does.

Environment data is a list of key-value pairs. Values are plain strings;
marking a pair as secret stores it encrypted (see
[Security](#security)).

## Security

Secrets are stored with the platform credential store: Keychain on macOS,
Credential Manager on Windows and Secret Service on Linux.
