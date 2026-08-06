# Windows 第二台机器与独立用户验收包

> 本验收包仅产生统一正式 1.0 的设备门禁证据，不构成内部 RC、Windows RC 或可分发候选版。验收通过后仍必须等待 macOS、签名公证和真实商务使用门禁全部关闭。

**权威事实日期：** 2026-07-29  
**当前验收基线程序版本：** `1.3.4`  
**升级来源版本：** `1.3.3`

## 目的

本验收包用于完成 1.0 候选版仍未关闭的两项 Windows 外部门禁：

- 在不同于构建机的第二台 Windows 机器上执行真实安装、跨版本升级、冷启动、卸载和隔离回滚。
- 使用不同于构建用户的 Windows 用户 SID 执行验收，并生成不包含原始机器标识和原始 SID 的哈希证据。

验收继续复用现有 `scripts/invoke-nsis-release-acceptance.ps1`，不创建第二套安装验收引擎、数据库、任务引擎或运行时。

## 在构建机生成可携带包

先执行只读计划检查：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\new-windows-secondary-machine-acceptance-bundle.ps1 -Version 1.3.4 -PreviousVersion 1.3.3 -FactDate 2026-07-29 -DryRun
```

确认输出的当前安装包 SHA-256 为 `176e3846199411a4a515fdacfdbb152ac541acd0d236ea48e6a6d8ca6307f5b3`，上一版本安装包 SHA-256 为 `c72692fe8fc13368575d1936cc0f213c10cbde3b1c69b8b93b62db7cfcf59da0`，Authenticode 为 `NotSigned` 后再生成 ZIP：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\new-windows-secondary-machine-acceptance-bundle.ps1 -Version 1.3.4 -PreviousVersion 1.3.3 -FactDate 2026-07-29
```

默认输出位于 `.runtime/windows-secondary-machine/`。ZIP 内包含当前安装包、上一版本安装包、现有 NSIS 验收引擎、目标机执行器、说明、bundle manifest 和 SHA-256 清单；不包含 API Key、R2 凭据、原始 MachineGuid 或原始用户 SID。

## 第二台 Windows 执行

1. 把 ZIP 复制到另一台 Windows 本地磁盘并解压，不要直接从网络共享或压缩包内运行。
2. 使用独立的本地或域 Windows 用户登录；默认门禁要求目标 MachineGuid 哈希和用户 SID 哈希均不同于构建机。
3. 先执行 dry-run：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\invoke-windows-secondary-machine-acceptance.ps1 -Mode Both -FactDate 2026-07-29 -ColdStartCount 20 -DryRun
```

4. dry-run 通过后执行真实验收：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\invoke-windows-secondary-machine-acceptance.ps1 -Mode Both -FactDate 2026-07-29 -ColdStartCount 20
```

干净测试账户不应使用 `-AllowExistingProductRegistration`。只有明确记录了既有产品注册状态且需要恢复时，才允许显式传入该开关。

## 通过判据

- bundle manifest、`SHA256SUMS.txt`、当前安装包和上一版本安装包全部实算匹配。
- `differentMachine=true`、`differentUser=true`，且证据仅记录 MachineGuid/SID 的 SHA-256。
- 成功摘要为 `status=passed`、`upgradeKind=cross-version-upgrade`、`1.3.3 → 1.3.4`、`coldStartCount=20`。
- 成功摘要的 `preflight`、`initial-install`、`data-backup`、`first-start`、`upgrade`、`restart`、`uninstall`、`registry-restore` 全部为 `passed`。
- 故障注入摘要为预期的 `status=failed`，且 `injectFailureAfterUpgrade=true`、`rollbackCompleted=true`、`rollbackError=null`、`uninstallCompleted=true`、`registryRestored=true`。
- SQLite、Vault、credentials、brain-workspace、business-workspace 哨兵以及文件集合、大小、SHA-256 在回滚后与升级前一致。
- `evidence/<run>/` 中生成 `secondary-machine-evidence.json`、升级摘要、回滚摘要和 `SHA256SUMS.txt`，并把整个证据目录带回发布归档审核。

## 仍不代表完成的事项

第二台 Windows 和独立用户验收通过后，仍需完成 macOS 签名、公证和真实启动验证，以及 Windows Authenticode 签名，才能宣称完整 1.0 RC 发布门禁完成。
