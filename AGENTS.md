# BSAIGC Desktop Engineering Rules

- This repository is an independent greenfield implementation. Do not copy from or depend on the company WIP repository.
- `upstream/openai-codex/` is a reproducible, ignored reference tree. Never edit or import application code from it directly.
- The official Codex app-server is the Agent Runtime boundary. Keep its generated protocol under `vendor/codex-app-server/<version>`.
- React components must not call Tauri `invoke` or `listen` directly. Only `src/client-sdk/DesktopHostAdapter.ts` may import Tauri APIs.
- Rust protocol types in `src-tauri/src/protocol.rs` are the BSAIGC protocol source of truth. Regenerate TypeScript with `pnpm protocol:generate`.
- UI receives stable IDs and lightweight JSON only. Do not expose credentials, provider headers, R2 secrets, or local absolute media paths.
- SQLite Ledger and local Vault are authoritative. Cloud sync, diagnostics, and Codex availability must not block local provider or project workflows.
- Preserve idempotency, revision CAS, event replay, and post-restart recovery for every durable command.
- New modules must declare commands, events, permissions, tools, and storage ownership before integration.
- Run `pnpm verify`, `cargo test --manifest-path src-tauri/Cargo.toml`, and the ignored real Codex handshake test before release.

<!-- BUSINESS-WORKBENCH-SCOPE:BEGIN -->
## Product Scope Lock: Business Workbench

- The only active product scope is the standalone desktop Business Workbench described in `docs/BUSINESS_WORKBENCH_SCOPE.md`.
- Build the contract-review vertical slice first, then customer/requirement, quotation, contract, payment request, acceptance, receivable/payment, ledger, and archive.
- Use a Codex Desktop-inspired panel workflow, but do not copy OpenAI branding or proprietary assets.
- Hide and freeze Creative Center, Infinite Canvas, media-generation pages, external CRM/Feishu integration, web deployment, e-signature, D1 business mirroring, and team sync unless the user explicitly reactivates them. R2 backup/restore remains in scope after the local contract-review slice.
- Keep SQLite Task Ledger and Local Vault authoritative. R2 is an asynchronous backup/restore replica only: local commit determines task success, R2 failure never blocks local work, and credentials remain behind the Rust Host Backup Adapter.
- Reuse mature open-source packages through the existing Client SDK and Rust Host. Do not add a second Agent Runtime, task engine, approval engine, permission authority, or business database.
- Do not delete unrelated shared code until dependency analysis proves it is not used by Task, Asset, Memory, Security, Artifact, Client SDK, or Codex Host. Hide first, prune later.
- Before accepting work, verify it directly advances a user-visible Business Workbench milestone. Avoid pure foundation work and unbounded repository research.
<!-- BUSINESS-WORKBENCH-SCOPE:END -->