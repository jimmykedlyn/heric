### Task 22: Documentation, status and the parity matrix

**Files:**
- Modify: `docs/book/{en,pt-BR,es}/src/getting-started.md`, `docs/book/{en,pt-BR,es}/src/requests-and-environments.md`
- Modify: `STATUS.md`, `PLAN.md`, `docs/inventory/FEATURES.md`
- Test: `bash scripts/check-docs-structure.sh`

- [ ] **Step 1:** Rewrite the getting-started page to match what M1 delivers: create a request, send it, read the response and the Console timeline. Do not describe environments, scripts, plugins or other protocols as available — they are M3 and later. Keep every heading identical in shape across the three languages.
- [ ] **Step 2:** In `docs/inventory/FEATURES.md`, move an M1 row to `Done` only where its task merged and its tests pass on all four targets. A row whose task was cut or deferred stays `In progress` with the deferral recorded in its Issue column. Never mark a row done for a task that did not land.
- [ ] **Step 3:** Update `STATUS.md` and `PLAN.md`'s M1 row.
- [ ] **Step 4:** Verify the five gates and the docs script.
- [ ] **Step 5: Commit** — `docs: describe M1 in three languages and close its parity rows`
