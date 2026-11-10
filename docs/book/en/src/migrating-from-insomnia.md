# Migrating from Insomnia

Planned for M2. Heric reads an existing Insomnia data directory in place and
imports every workspace, environment, folder, request and cookie jar without
loss. The data directory lives at:

| Platform | Path |
|---|---|
| Windows | `%APPDATA%\Insomnia` |
| Linux | `~/.config/Insomnia` |
| macOS | `~/Library/Application Support/Insomnia` |

Migration runs once per import and never writes to the original directory.
