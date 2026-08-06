# Windows 1.0 候选版发布安全检查表

> 自 2026-08-02 起，文件名和历史术语中的 `RC` 仅表示 Windows 验收阶段，不构成独立发布节点。此清单通过后仍禁止分发，必须等待 macOS、双机 Windows、签名公证和真实商务验收全部通过后统一发布正式 1.0。

**检查日期：** 2026-07-29  
**适用范围：** Windows NSIS 候选版；不代表已签名生产发布  
**当前仓库版本：** `1.3.4`（必须由 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三方一致证明）

## 发布结论门禁

- [ ] 版本一致性通过：三处版本相同，安装包名称为 `半山商务工作台_<version>_x64-setup.exe`。
- [ ] 构建在 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0` 下完成。
- [ ] 构建脚本未读取或注入内部 API Key；`BSAIGC_INTERNAL_API_KEY` 必须为空。
- [ ] `src-tauri/resources/r2.config.json` 存在但 `accessKeyId`、`secretAccessKey` 均为空；真实 R2 凭据只允许存在于下载运行时步骤的临时环境。
- [ ] 产物未进入 Git：`.runtime/`、`release/`、安装包、运行时二进制和本地 R2 配置均被忽略或未跟踪。
- [ ] 构建日志已生成并记录 SHA-256；日志不得包含凭据赋值或密钥值。
- [ ] 安装包存在配套 `.sha256`、`.unsigned.txt`、`build-manifest.json`。
- [ ] Authenticode 明确为 `NotSigned`，并在所有发布材料中标识“未签名”；不得把候选版表述为已签名版本。
- [ ] 来源快照通过现有 `create-source-snapshot.ps1` 的敏感信息门禁，包含 `source-manifest.json`、源码 ZIP 和快照 SHA-256。
- [ ] 发布目录包含 `release-manifest.json` 与 `SHA256SUMS.txt`；所有清单中的哈希可复算。
- [ ] NSIS 安装、启动、升级、重启、卸载验收通过；验收不使用 `-ExpectEmbeddedPreviewCredential`。
- [ ] 本轮不执行 R2 发布上传；只保留 GitHub Actions review artifact，正式上传必须另行审批并使用最小权限凭据。

## 标准执行顺序

```powershell
$env:CARGO_BUILD_JOBS = "1"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
$env:CARGO_INCREMENTAL = "0"

pnpm install --frozen-lockfile
pnpm skills:validate
pnpm codex:sidecar:verify
./scripts/build-internal-preview.ps1

$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
./scripts/invoke-nsis-release-acceptance.ps1 `
  -InstallerPath "src-tauri/target/release/bundle/nsis/半山商务工作台_${version}_x64-setup.exe" `
  -Version $version `
  -RunId "windows-rc-$version"
./scripts/package-release.ps1 -Version $version -ReleaseCandidate
```

## 必查文件

候选版 `release/<version>/` 至少应包含：

- `banshan-workbench-v<version>-setup-unsigned.exe`
- `banshan-workbench-v<version>-source.zip`
- `source-manifest.json`
- `build-windows-<version>.log`
- `build-manifest.json`
- `UNSIGNED.txt`
- `release-manifest.json`
- `SHA256SUMS.txt`

## 安全审计记录

- 原工作流曾将 R2 `accessKeyId`、`secretAccessKey` 写入打包资源；现改为公开配置空凭据，下载运行时的 R2 Secret 仅在单个下载步骤作用域内可见。
- 原工作流曾把内部预览 Key 传入 Tauri 构建；现候选版构建默认拒绝环境变量和旧 `.runtime` 配置中的 API Key。
- 发布脚本拒绝覆盖已有 `release/<version>` 目录，避免候选材料被静默替换。
- 发布脚本复用现有源码快照工具，不创建第二套扫描器、数据库、任务引擎或运行时。
- 候选版不自动写入 R2；GitHub artifact 只用于人工审核和下载。

## 当前阻塞与处理

2026-07-29 审计时，工作区中的本地 `src-tauri/resources/r2.config.json` 含有非空凭据字段。安全脚本会故意 fail-fast，且不会打印或复制这些值；运行候选版构建前需在隔离构建环境生成空凭据版本。该文件已被 Git 忽略，不得 `git add`。

本地工作树还存在与本任务无关的并行改动；本检查表不要求清理、回滚、提交或推送这些改动。源码快照是否可发布，以现有敏感信息扫描、Git diff 扫描和 `source-manifest.json` 为准。

## 第二台 Windows 与独立用户携带验收

- [ ] 使用 `scripts/new-windows-secondary-machine-acceptance-bundle.ps1` 从 `release/1.3.4`、`release/1.3.3` 和现有 NSIS 验收引擎生成可携带 ZIP；先执行 `-DryRun`，不得手工拼装安装包和脚本。
- [ ] 在不同 Windows MachineGuid 的第二台机器上、使用不同 Windows 用户 SID 执行 `scripts/invoke-windows-secondary-machine-acceptance.ps1 -Mode Both -ColdStartCount 20`。
- [ ] 目标机必须先复算 bundle manifest 与 `SHA256SUMS.txt`，并保持候选安装包 Authenticode 为已披露的 `NotSigned`；不得使用 `-ExpectEmbeddedPreviewCredential`。
- [ ] 成功跨版本摘要和预期故障回滚摘要必须连同 `secondary-machine-evidence.json`、证据目录 `SHA256SUMS.txt` 一并带回；证据只保存 MachineGuid/SID 哈希，不保存原始标识。
- [ ] 详细命令、通过字段和回传内容以 `docs/WINDOWS_SECONDARY_MACHINE_ACCEPTANCE_20260729.md` 为准。该外部实机验收未执行前，不得勾选第二台 Windows 或独立用户门禁。
