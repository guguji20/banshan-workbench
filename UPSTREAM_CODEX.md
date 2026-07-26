# Official OpenAI Codex pin

This project uses the official Codex app-server as its Agent Runtime boundary. It does not maintain a second custom Codex core.

## Pinned revision

- Repository: `openai/codex`
- Release tag: `rust-v0.144.5`
- Codex CLI and app-server runtime: `codex-cli 0.144.5`
- Source archive: `https://codeload.github.com/openai/codex/zip/refs/tags/rust-v0.144.5`
- Source archive SHA-256: `d4398b3652ca7974428c4de46d0e1ebb8793ccb7c65f52b05a7a55078ec49fb5`
- Pin date: `2026-07-18`

## Windows native sidecar

The Windows desktop bundle includes the official x64 native npm platform artifact:

- Platform package: `@openai/codex@0.144.5-win32-x64`
- Target: `x86_64-pc-windows-msvc`
- Official registry: `https://registry.npmjs.org/`
- Tarball: `https://registry.npmjs.org/@openai/codex/-/codex-0.144.5-win32-x64.tgz`
- Tarball SHA-1: `b9d63532a8cb0e113625c3c9ed0f14b669f50e87`
- Tarball integrity: `sha512-DnsSTlnnzleTxvLwIGnBitKInscxn2I7qASqosS8Fv+qysBygd+ZiBn/SQsRCgQ28PAlsNzmd3Gf3ZTecolAmg==`
- Entrypoint: `src-tauri/resources/codex-runtime/codex.exe`
- Entrypoint size: `341195568` bytes
- Entrypoint SHA-256: `efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b70e22a`
- Authenticode signer: `OpenAI OpCo, LLC`
- Signer certificate thumbprint: `838CD705CC1344F84DAF4A7479BD532445B3ABED`

`src-tauri/resources/codex-runtime/manifest.json` records the version, source, npm integrity, signer, size and SHA-256 for every bundled runtime file. The runtime directory also carries the official Windows command-runner, sandbox setup helper and ripgrep binary. `rg.exe` is copied next to `codex.exe` in addition to the official `codex-path/` location so the root-level Tauri sidecar layout remains usable without changing the Rust runtime discovery contract.

Tauri bundles the complete directory through:

```json
"resources/codex-runtime/"
```

The installed path remains `$RESOURCES/resources/codex-runtime/`, which matches the existing Rust bundled-resource lookup.

## Reproduce and verify

Prepare the runtime from an already installed, hash-matching official npm platform package. If no valid local package is available, the script immediately falls back to the pinned npmjs tarball and verifies its SHA-1, SHA-512 integrity, file sizes, SHA-256 values, CLI version and Authenticode signer before copying anything.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-codex-sidecar.ps1
```

Force a clean official npmjs fetch:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-codex-sidecar.ps1 -ForceDownload
```

Verify the checked-in/runtime files and Tauri resource declaration without downloading:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-codex-sidecar.ps1
```

The native entrypoint is larger than GitHub's normal single-file limit. If the executable is committed to a hosted Git repository, that transport must use Git LFS or an equivalent release-artifact mechanism. The desktop build itself reads the local pinned resource and does not depend on a personal `%USERPROFILE%\.codex` installation.

## Boundaries

- `upstream/openai-codex/` is an ignored, reproducible reference/build working tree. Do not edit it.
- `vendor/openai-codex/rust-v0.144.5/` contains the upstream Apache-2.0 license and notice.
- `vendor/codex-app-server/v0.144.5/typescript/` contains generated official TypeScript protocol definitions.
- `vendor/codex-app-server/v0.144.5/json-schema/` contains generated official JSON Schemas.
- Application code talks to `codex app-server` over its default JSONL stdio transport.
- The application must send `initialize`, wait for its response, then send `initialized`.
- Product `CODEX_HOME`, Provider credentials and user data remain isolated from personal Codex installations.

## Refresh

```powershell
powershell -ExecutionPolicy Bypass -File scripts/refresh-openai-codex.ps1
pnpm codex:generate
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/prepare-codex-sidecar.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-codex-sidecar.ps1
```

A version upgrade is a deliberate protocol and runtime migration: update the pin, regenerate both protocol bundles, review the schema diff, refresh all sidecar hashes and npm integrity values, run all protocol and runtime tests, and only then change the runtime version.