English | [Português](README.pt-BR.md) | [Español](README.es.md)

# Heric

A native API client in Rust with an egui/eframe interface, compatible with
Insomnia data and workflows.

> Heric is an independent open source project. It is not affiliated with,
> endorsed by, or sponsored by Kong Inc. It is inspired by, and aims to be
> compatible with, the Insomnia API client.

## Status

Early development. The repository currently contains the workspace skeleton,
CI configuration and the project plan. The first usable release, with a real
request round trip, is planned for milestone M1.

## Goals

- Feature parity with the original client: HTTP, GraphQL, gRPC, WebSocket,
  SSE, environments, template tags, scripting, import/export and code
  generation.
- Read existing Insomnia data directories and import/export the Insomnia
  format without loss.
- Run natively on Windows (x86_64), Linux (x86_64) and macOS (Apple Silicon
  and Intel).
- Keep the UI responsive: request execution runs off the UI thread.

## Building

Prerequisites: a Rust toolchain (see `rust-toolchain.toml`) and, on Linux,
the development packages listed in the documentation.

```bash
git clone https://github.com/jimmykedlyn/Heric.git
cd Heric
cargo build --workspace
```

Run the desktop application:

```bash
cargo run -p heric
```

Run the command line interface:

```bash
cargo run -p heric-cli --version
```

## Repository layout

```
crates/     reusable libraries (model, storage, template, http, auth, ...)
apps/       binaries: gui (heric) and cli (heric-cli)
docs/       architecture decisions, plans, inventory and the mdBook sites
```

## Documentation

- English: https://heric.github.io/Heric/en/
- Português: https://heric.github.io/Heric/pt-BR/
- Español: https://heric.github.io/Heric/es/

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The project uses Gitflow: `develop`
is the default branch and every change arrives through a pull request.

## License

MIT. See [LICENSE](LICENSE).

Parts of this project are derived from Insomnia (Kong Inc.), licensed under
the Apache License 2.0. See [NOTICE](NOTICE) and
[licenses/APACHE-2.0-insomnia.txt](licenses/APACHE-2.0-insomnia.txt).
