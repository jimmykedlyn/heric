### Task 18: A working `heric-cli run`

**Files:**
- Modify: `apps/cli/src/main.rs`, `apps/cli/Cargo.toml`
- Test: `#[cfg(test)]` in `main.rs`, plus `apps/cli/tests/run.rs`

**Interfaces:**
- Produces: `Command::Run` printing a status line, the headers and the body; exit code `0` for any HTTP status, `1` for a transport or configuration failure; a top-level `--data-dir` flag.

- [ ] **Step 1: Write the failing tests** — `run_prints_the_status_and_body`, `run_exits_one_when_the_host_is_unreachable`, `run_accepts_a_data_dir_override`.
- [ ] **Step 2–5:** add `tokio` to the CLI crate; start the core with `Core::start(CoreBuilder::new(paths).build())`, take `core.handle()`, load the request from the store through the document service, `handle.block_on(handle.requests().send_and_wait(id))`, print. Replace the `pending("run", …)` bail; leave the other subcommands reporting their milestone. The `Core` stays alive for the whole command, because dropping it stops the runtime. Verify against a loopback listener; commit `feat(cli): send a request and print the response`.

