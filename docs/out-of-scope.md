# Out of scope

The following features are intentionally not implemented. Interfaces stay as
stubs so they can be added later without breaking the data model.

## Cloud sync

Syncing collections to the Insomnia cloud account. The `heric-core` crate
exposes a `CloudSync` trait stub, and the storage model keeps the fields
needed to round-trip cloud-linked workspaces, but no network calls are made.

## Kong account login

Login, session and subscription checks against Kong's services. The GUI shows
a disabled account area with an explanation. No credentials are collected.

## Telemetry

The original client sends analytics events. Heric does not. There is no
analytics crate and no outbound call other than the requests the user makes.
