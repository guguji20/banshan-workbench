# 可复现源码快照

`create-source-snapshot.ps1` 用于把大量未提交 WIP 固化为可审计、可校验的内测源码快照。它不会修改或删除仓库文件。

## 输出内容

每次成功执行会创建独立目录，包含：

- `source-manifest.json`：UTF-8（无 BOM）清单；逐文件记录相对路径、字节数、SHA-256 和 Git 状态。
- `git-diff.binary.patch`：相对当前 `HEAD` 的完整 binary patch（不含明确排除项）。
- `untracked-files.txt`：本次源码 ZIP 中包含的 Git untracked 文件。
- `git-status.txt`：创建快照时的分支和 working tree 状态。
- `huabang-business-system-source-<version>-<head>-<utc>.zip`：按相对路径排序、固定 ZIP 时间戳的源码包。
- `SHA256SUMS.txt`：全部交付文件 SHA-256 与稳定的 `FINAL_SNAPSHOT_SHA256`。

## 默认排除

不会进入源码 ZIP：

- `.git/`
- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `release/`
- `.runtime/`
- `upstream/` 及其他嵌套 Git 克隆
- 编译产物与归档二进制（EXE、DLL、PDB、ZIP 等）
- 超过阈值的二进制文件（默认 20 MiB）
- reparse point / symlink
- 输出目录自身

被排除的单文件会在 manifest 中记录原因、大小和 SHA-256（适用时）。

## 敏感信息阻断

脚本会在源码文件以及 Git diff 中检测高置信度敏感内容，包括：

- OpenAI 风格、GitHub、AWS、Google、Slack token；
- private key 头；
- 高熵的 API key、token、password、client secret 字面量；
- `.env`、私钥和凭据文件名。

发现疑似敏感信息时立即失败，控制台只输出文件、行号和规则，永不打印命中的值。明显的 test/fake/placeholder 测试值不会误阻断。

## Dry run

Windows PowerShell 5.1：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\create-source-snapshot.ps1 `
  -Version 1.2.0 `
  -OutputDirectory .\.runtime\source-snapshot-validation `
  -DryRun
```

Dry run 会完成 Git 读取、文件枚举、哈希和敏感信息检查，但不会保留最终快照。

## 创建正式快照

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\create-source-snapshot.ps1 `
  -Version 1.2.0 `
  -OutputDirectory .\.runtime\source-snapshot-validation
```

成功标志：

```text
Sensitive findings: 0
FINAL_SNAPSHOT_SHA256: <sha256>
SOURCE_ZIP_SHA256: <sha256>
MANIFEST_SHA256: <sha256>
SNAPSHOT_OK
```

## 复核

```powershell
$dir = Get-ChildItem .\.runtime\source-snapshot-validation -Directory |
  Sort-Object LastWriteTime -Descending |
  Select-Object -First 1

Get-Content -LiteralPath (Join-Path $dir.FullName 'SHA256SUMS.txt')
Get-Content -LiteralPath (Join-Path $dir.FullName 'source-manifest.json') -Raw -Encoding UTF8 |
  ConvertFrom-Json |
  Select-Object version, repository, snapshot, security
```

`FINAL_SNAPSHOT_SHA256` 绑定版本、Git HEAD、源码树、binary patch 和 untracked 清单；同一内容可据此核对。源码 ZIP 还会在落盘后逐 entry 重新计算大小和 SHA-256，任一不一致都会阻断交付并清理仅由本次运行创建的 staging 目录。
