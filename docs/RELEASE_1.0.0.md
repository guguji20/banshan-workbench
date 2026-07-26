# 半山商务工作台 1.0.0 发布说明

- 产品：半山商务工作台
- 版本：`1.0.0`
- 发布日期：`2026-07-22`
- 当前状态：**正式内测发布，最终安装包验收通过**

## 1. 发布范围

1.0 仅交付本地桌面商务工作台，主流程为：

```text
客户/项目
→ 需求确认
→ 报价
→ 合同导入、解析与审查
→ Evidence 与人工确认
→ 审查报告
→ 正式合同
→ 请款与付款计划
→ 验收
→ 到账/回款
→ 台账与归档
```

以下能力不属于 1.0 发布范围，保持隐藏或冻结：

- 创意中心；
- 无限画布；
- 图片、视频、音频媒体生成；
- 销帮帮、飞书及其他外部 CRM/协同系统；
- 正式网页版、D1 业务镜像、团队同步和跨设备协作。

`WebHostAdapter` 仅保留协议位置，不代表网页版已经交付。

## 2. 本地权威与容灾保证

### SQLite 与 Local Vault

- SQLite 是任务、命令、事件和商务记录的本地权威。
- Local Vault 是合同原件、附件、报告和生成文件的本地主存储权威。
- 本地提交成功才决定任务成功；UI 只接收稳定 `assetId` 和轻量状态。
- 应用重启后从本地 Ledger、SQLite 和 Vault 恢复。

### R2 异步备份

- R2 只做异步灾备副本，不是业务数据库、任务权威或主读取源。
- R2 未配置、断网或上传失败不得阻断本地业务成功。
- 失败项保留在 Durable Backup Outbox，允许后续重试。
- 恢复时必须校验 ETag、大小、元数据和 SHA-256，再无覆盖写回 Local Vault。

### Codex Runtime

- 官方 Codex app-server 是唯一 Agent Runtime。
- Thread、Turn、Interrupt、Tool、Approval 和流式事件由 Rust Host 管理。
- 模型与本地项目、历史、业务记录和人工决策分离。
- 未配置 AI 凭据或断网时，确定性规则、Evidence、人工确认和本地报告仍应可用。

## 3. 安装包与签名状态

最终安装包已通过完整质量门和 NSIS 生命周期验收。

- 预期文件名：`半山商务工作台_1.0.0_x64-setup.exe`
- 最终安装包 SHA-256：`0be38deebc31c3b32a0f2d7807ae3d1a098fa7c4d80f27b9371c58ae5cf3911e`
- 产品安装包 Authenticode：**NotSigned（未签名）**
- 安装方式：NSIS，当前用户安装
- Windows 可能显示未知发布者或 SmartScreen 提示。

> 上述 SHA-256 来自 2026-07-22 完成质量门、最终 NSIS 构建和真实升级验收后的安装包；发布目录会再次独立核对。

## 4. Codex Sidecar 发行信息

- Codex 版本：`0.144.5`
- Target：`x86_64-pc-windows-msvc`
- `codex.exe` SHA-256：`efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b70e22a`
- Manifest SHA-256：`7442f28332324cc613317fa3bd315ebc8c1d967ff1b131144a70ddb337030abe`
- Authenticode：`Valid`
- Signer：`OpenAI OpCo, LLC`
- Thumbprint：`838CD705CC1344F84DAF4A7479BD532445B3ABED`
- 许可证：`Apache-2.0`
- 安装包必须同时分发：`resources/codex-runtime/LICENSE` 与 `resources/codex-runtime/NOTICE`

产品 NSIS 自身未签名，与官方 Codex sidecar 的有效 Authenticode 签名是两件独立事项，不得混淆。
### 3.1 内测 AI 默认配置

- 内测安装包在编译阶段注入半山 AIGC 的默认 Provider、API 地址、模型和内测凭据，源码与仓库文件不保存明文密钥。
- 首次启动后，凭据立即写入当前 Windows 用户的 DPAPI 保护文件；界面、日志、事件和诊断只返回脱敏状态。
- 设置中心允许新增、切换、停用或删除其他 OpenAI-compatible Provider，并可随时更换默认模型。
- 用户已经保存过的 Provider 设置优先于安装包默认值；覆盖安装不会覆盖现有选择。
- 该默认凭据只用于受控内测，正式对外发行前必须撤销或轮换，并改为组织级安全下发。

## 5. Windows NSIS 自动验收

自动化入口：

```text
scripts/invoke-nsis-release-acceptance.ps1
```

脚本覆盖以下生命周期：

1. 校验最终安装包文件名、存在性和 `NotSigned` 状态；
2. 将初始版本静默安装到 `.runtime/nsis-acceptance/<RunId>/install`；
3. 使用隔离的 `USERPROFILE`、`APPDATA`、`LOCALAPPDATA`、`TEMP` 和 `CODEX_HOME` 启动应用；
4. 确认隔离 Profile 内生成 SQLite Ledger 与 Local Vault；
5. 写入只属于测试 Profile 的 Ledger/Vault 数据保留哨兵，并生成安装前后 SHA-256 快照；
6. 使用最终安装包覆盖安装，验证升级安装没有改写 SQLite/Vault；
7. 校验安装目录中的 Codex `LICENSE`、`NOTICE`、文件大小和 SHA-256；
8. 完成两次独立 PID 的应用启动、退出和重启后数据保留检查；
9. 静默卸载，校验主程序、卸载程序和测试注册表项已移除；
10. 确认卸载后 SQLite/Vault 与数据保留哨兵仍存在；
11. 输出 UTF-8 无 BOM 日志、数据快照和 JSON 验收摘要。

### 5.1 Dry-run

Dry-run 只做预检和计划输出，不安装、不启动进程、不修改注册表、不创建验收目录：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ".\scripts\invoke-nsis-release-acceptance.ps1" `
  -InstallerPath ".\src-tauri\target\release\bundle\nsis\半山商务工作台_1.0.0_x64-setup.exe" `
  -RunId "1.0.0-rc-dry-run" `
  -DryRun
```

### 5.2 旧构建覆盖升级验收

要证明“升级保留”而不是仅验证全新安装，必须提供同一产品的旧构建安装包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ".\scripts\invoke-nsis-release-acceptance.ps1" `
  -PreviousInstallerPath ".\.runtime\previous-installer\半山商务工作台_1.0.0_pre-receivables_x64-setup.exe" `
  -InstallerPath ".\src-tauri\target\release\bundle\nsis\半山商务工作台_1.0.0_x64-setup.exe" `
  -Version "1.0.0" `
  -RunId "1.0.0-upgrade-rc1"
```

如果不传 `PreviousInstallerPath`，脚本只验证同一安装包的重装与数据保留。1.0 最终验收使用的是同一产品、同一语义版本的旧构建，因此验收摘要标记为 `same-version-replacement`；它真实覆盖了旧 SQLite schema 到最终 schema 的迁移，但不冒充不同语义版本之间的升级。

### 5.3 安全限制

- 所有测试安装、Profile、日志、注册表备份和验收结果必须严格位于仓库 `.runtime` 下。
- 路径存在重解析点、junction 或越过 `.runtime` 时立即拒绝执行。
- 已存在同名产品注册时，默认拒绝覆盖，避免安装器改动现有安装目录。
- 只有明确传入 `-AllowExistingProductRegistration` 才会备份并在结束时恢复既有卸载注册信息；推荐仍在干净 Windows 测试账号或 VM 中运行。
- 脚本不会递归删除测试目录，也不会删除非测试目录；卸载只调用测试安装目录内的 NSIS `uninstall.exe`。
- 残留注册表清理仅允许命中“安装位置明确等于本次测试安装目录”的 HKCU 卸载项。
- 进程清理仅允许终止可执行文件路径位于本次测试安装目录内的进程。
- 即使中途失败，脚本也会尝试受限卸载、清理测试注册表项、恢复既有注册表快照，并保留日志与隔离 Profile 供排查。

### 5.4 输出目录

真实执行后的证据位于：

```text
.runtime/nsis-acceptance/<RunId>/
  acceptance-summary.json
  data-before-upgrade.json
  data-after-upgrade-before-launch.json
  logs/acceptance.log
  profile/
  registry-backup/
```

隔离 `profile/` 默认保留，用于证明卸载没有删除 SQLite/Vault。脚本不自动删除该目录。

### 5.5 最终通过记录

- 验收时间：`2026-07-22`
- RunId：`release-1.0.0-20260722-095842`
- 结果目录：`.runtime/nsis-acceptance/release-1.0.0-20260722-095842`
- 旧构建 SHA-256：`bd437456b14848c26221cd0fe26320d0f615729715d2a2ca9ff1496c6d96e334`
- 最终安装包 SHA-256：`0be38deebc31c3b32a0f2d7807ae3d1a098fa7c4d80f27b9371c58ae5cf3911e`
- 结果：旧包安装与首启、SQLite/Vault 建立、最终包覆盖安装、旧库迁移、两次独立重启、Codex LICENSE/NOTICE、数据哨兵、静默卸载、测试注册表清理和既有注册表恢复全部通过。

## 6. 发布质量门

最终构建前必须全部通过：

```powershell
pnpm protocol:generate
pnpm check
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-codex-sidecar.ps1
git diff --check
pnpm release:build
```

构建后的最终 NSIS 还必须执行第 5 节的真实安装生命周期验收。

## 7. 发布验收清单

### 已完成

- [x] 产品、版本和发布范围收敛到商务工作台；
- [x] SQLite/Vault 本地权威和 R2 异步灾备原则已明确；
- [x] Codex sidecar Manifest 已纳入 `LICENSE` 与 `NOTICE`；
- [x] 产品 NSIS 未签名状态已明确披露；
- [x] Windows NSIS 安装/升级/重启/卸载/注册表/数据保留自动化脚本已建立；
- [x] 自动化具备 dry-run、受限路径、明确日志和失败清理设计。

### 最终发布前必须完成

- [x] 完整质量门全部通过；
- [x] 使用最终新构建安装包执行真实 NSIS 生命周期验收；
- [x] 使用同一产品旧构建安装包完成旧库覆盖升级与数据保留验收；
- [x] 确认安装目录存在 Codex `LICENSE` 与 `NOTICE`；
- [x] 确认应用重启后项目、合同、报告、付款、验收、回款和归档记录仍可读取；
- [x] 确认卸载退出码为 0、安装文件和测试注册表项清理、SQLite/Vault 默认保留；
- [x] 完成断网、R2 未配置和 R2 失败不阻断本地成功验收；
- [x] 完成三份真实中文 DOCX 合同闭环回归；
- [x] 计算最终安装包实际 SHA-256，替换本文唯一占位；
- [x] 运行 `scripts/package-release.ps1` 生成最终发布目录。

## 8. 最终发布目录

验收通过并替换 SHA-256 占位后，执行发布打包。最终目录只能包含：

```text
release/1.0.0/
  半山商务工作台_1.0.0_x64-setup.exe
  SHA256SUMS.txt
  RELEASE_1.0.0.md
```

第 7 节必选项已全部完成；本文件与 `SHA256SUMS.txt`、最终安装包共同构成 1.0.0 发布材料。
