# macOS Release Readiness — 2026-07-29

> 自 2026-08-02 起，macOS 构建不再作为可独立分发的平台 RC；只有与同一 Git 提交的 Windows 产物、双机验收、签名公证和真实商务使用全部通过后，才能进入唯一正式 1.0 发布批次。

## 结论

当前状态：**静态发布工作流已加固，但 macOS 1.0 候选版仍不可宣称就绪。**

本轮写集仅限 `.github/workflows/build-macos.yml` 和本文件，未修改 `src-tauri/tauri.conf.json`、脚本、业务代码、旧 `App.tsx`、执行报告或 master plan。Windows 主机无法替代真实 macOS 构建、签名、公证和启动验证。

## 已完成的静态门禁

- 架构固定为 `aarch64-apple-darwin`，runner 固定为 GitHub ARM64 `macos-15`，并在构建前验证 `uname -m = arm64`。
- Rust 构建固定 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`。
- `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处版本必须一致。
- Codex Runtime 固定 `0.144.5`，下载包必须匹配 `CODEX_DARWIN_ARM64_SHA256` 后才能解压。
- 不再把 R2 上传密钥写入 `r2.config.json`；运行时资源只保留公开 endpoint、bucket 和更新清单 URL。
- 不再把 `BSAIGC_INTERNAL_API_KEY` 作为编译期环境变量写入候选版二进制。
- 增加 Developer ID 证书导入、签名 identity 验证、Apple 公证环境变量和独立临时 keychain。
- 构建前执行现有 `pnpm release:verify`，Rust 作业数继承统一限制。
- 构建产物必须通过 `codesign`、Gatekeeper `spctl`、`stapler` 验证。
- 启动烟测要求应用进程保持存活 12 秒，并保存日志。
- DMG 使用不可变名称 `huabang-business-system-{version}-aarch64-apple-darwin-{commit12}.dmg`，避免同版本 workflow 重跑覆盖。
- 真实 runner 在构建、签名、公证、Gatekeeper、stapler、sidecar 和 12 秒启动烟测全部成功后，生成状态为 `passed` 的 `macos-release-manifest.json`；当前工作流不生成名为 `macos-acceptance-summary.json` 的独立文件。
- 同步生成 `macos-build.log`、`macos-gate-checks.log`、`macos-smoke.log`、`macos-release-manifest.json`、DMG 单文件 SHA 和 `SHA256SUMS-macos.txt`；`pnpm verify:macos-release-evidence` 会校验这些文件的集合、大小和 SHA-256。
- GitHub Artifact 只留存禁止分发的 DMG、构建/门禁/启动日志、发布 manifest 和校验文件；Artifact 名称包含版本、target 和完整 commit。
- Artifact 上传前执行硬门禁 `pnpm verify:macos-release-evidence`；校验失败时 workflow 直接失败，不允许绕过证据校验发布。
- R2 发布改为显式 `publish` 开关；证据上传到 `evidence/macos/{version}/{commit}/{runId}-{attempt}` 不可变前缀，再更新根 `version-mac.json`。
- 覆盖更新清单前先确认旧清单存在，再备份到 `rollback/`；上传后下载 DMG、SHA、全部证据和根清单，执行清单比对与 `SHA256SUMS-macos.txt` 全量复核。
- 工作流设置 `contents: read`、禁止 checkout 持久凭据、并发互斥、90 分钟超时和 `macos-release` Environment 门禁。

## 证据产物与上传链

以下文件只会在真实 GitHub Apple Silicon runner 已完成既有签名、公证、Gatekeeper、stapler、sidecar 与启动步骤后生成；本次 Windows 静态审计没有生成真实通过证据：

- `macos-build.log`：保留 Tauri 构建、签名和公证命令的原始 runner 输出。
- `macos-gate-checks.log`：保留 `codesign`、App/DMG `spctl` 和 App/DMG `stapler` 的直接验证输出。
- `macos-release-manifest.json`：记录产品、版本、target、commit、workflow、门禁状态、日志记录和各发布文件 SHA/大小。
- `SHA256SUMS-macos.txt`：覆盖 DMG、DMG 单文件 SHA、构建日志、门禁日志、启动日志和发布 manifest。

正式推广 workflow 才会在所有外部门禁通过后生成并发布 `version-mac.json`；当前 `build-macos.yml` 只上传禁止分发的 GitHub Artifact，不更新 R2。正式 R2 发布顺序为：备份旧根清单、上传不可变 DMG/证据、最后更新根清单、重新下载全部对象并复核。任何上传、下载、`cmp` 或 SHA 校验失败都会使发布 job 非零退出。

## 已验证

- `.github/workflows/build-macos.yml`：`yaml-lint` 通过。
- `src-tauri/tauri.conf.json`：JSON 解析通过。
- 指定文件 `git diff --check`：通过。
- UTF-8 无 BOM：通过。
- 未在 Windows 上伪称完成 macOS 编译、签名、公证或启动。

## 阻塞 1：Codex Runtime 真实 macOS 验证

工作树已补齐标准 macOS App Bundle 资源发现路径：`Contents/MacOS/<app>` 启动时会检查 `Contents/Resources/codex-runtime/codex`，并有对应 Rust 回归测试。该修复已在 Windows 主机上通过 `cargo fmt --check`、`cargo check`、目标回归测试和 `pnpm release:verify`。

仍未完成的是**真实 macOS ARM64** 证据：需要在 `macos-15` Apple Silicon runner 上完成 Codex sidecar 启动、Brain 基础任务、签名、公证、Stapler、Gatekeeper 和冷启动烟测。Windows 主机不能替代这些验证。
## 阻塞 2：媒体运行时只有 Windows 二进制

`src-tauri/resources/media-runtime` 当前只有 `ffmpeg.exe`、`ffprobe.exe` 和 Windows DLL；macOS workflow 会清空该目录并写入 `CAPABILITY_DISABLED.txt`，不会把 Windows 二进制带入 macOS 包。若 1.0 macOS 验收包含视频/媒体处理，此项必须补齐签名兼容的 ARM64 macOS runtime、许可证文件、固定 SHA 和启动探测；否则必须明确把该能力标为 macOS 暂不可用，而不能静默失败。

## 阻塞 3：外部 secrets 与账号门禁

GitHub Environment `macos-release` 必须配置并验证：

- `APPLE_CERTIFICATE`：Developer ID Application `.p12` 的 Base64。
- `APPLE_CERTIFICATE_PASSWORD`。
- `APPLE_SIGNING_IDENTITY`。
- `APPLE_ID`、`APPLE_PASSWORD`（app-specific password）、`APPLE_TEAM_ID`。
- `KEYCHAIN_PASSWORD`。
- `CODEX_DARWIN_ARM64_SHA256`：0.144.5 ARM64 tarball 的固定 SHA-256。
- 仅发布时需要 `R2_S3_ACCESS_KEY_ID`、`R2_S3_SECRET_ACCESS_KEY`，权限应限制到 `banshan-releases` 发布前缀。

还需确认 Apple Developer 证书未过期、Team ID 正确、公证协议已接受、GitHub ARM64 runner 可用、R2 公网域名和对象读取正常。

`pnpm verify:macos-release-evidence` 与对应测试已接入，会校验证据 schema、事实日期、版本/commit/run 一致性、文件集合、SHA 和大小；真实 runner 仍必须生成有效证据后才能通过上传前门禁。

## 阻塞 4：真实候选版验收

首次 `publish=false` 构建必须在 GitHub ARM64 runner 完成以下证据后，才允许第二次 `publish=true`：

1. workflow 全绿，下载 GitHub Artifact 并核对 SHA-256。
2. 在干净 Apple Silicon Mac 安装 DMG，Gatekeeper 无拦截。
3. 冷启动至少 5 次；正式 1.0 门禁按主计划完成 macOS 启动和基础任务烟测。
4. 登录、本地 SQLite/Vault、文件导入、报价或验收最小任务、Codex Brain 基础 turn 可运行。
5. 升级前备份应用数据目录；从上一版本升级后数据、事件 revision 和文件资产可读。
6. 用 `rollback/version-mac-before-{run}.json` 恢复旧更新清单，并验证旧 DMG 仍可下载、SHA 正确。
7. 不允许用覆盖同名 DMG 的方式回滚；回滚只切换清单到已验证的不可变版本。

## Tauri 配置审计

- `identifier`、图标和标准 resources 声明可用于 macOS 构建。
- CLI 的 `--bundles dmg` 会显式选择 DMG，本轮无需把全局 `bundle.targets` 从 Windows `nsis` 改成跨平台列表。
- `tauri.conf.json` 的产品名、窗口标题、短描述和长描述已统一为“华邦互娱商务系统”及跨平台商务系统语义；保留既有 `identifier` 以维持升级兼容。
- Codex 资源发现路径已通过代码修复；macOS ARM64 FFmpeg/FFprobe runtime 仍缺失，继续作为媒体能力候选版阻塞。

## 发布判定

- 静态配置门禁：**通过**。
- 证据 schema 与上传链：**校验命令和测试已接入，等待真实 runner 生成正式证据**。
- ARM64 macOS 云构建：**未执行**。
- Developer ID 签名与 Apple 公证：**未验证**。
- macOS 启动烟测：**未验证**。
- Codex Brain 基础任务：**等待真实 macOS ARM64 runner 与凭据完成验证**。
- macOS 媒体能力：**阻塞于 ARM64 runtime 缺失**。
- 1.0 macOS 候选版发布：**不通过，禁止宣称已完成**。
