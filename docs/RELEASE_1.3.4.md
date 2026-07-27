# 半山商务工作台 1.3.4 更新说明

- 版本：`1.3.4`
- 日期：`2026-07-28`
- 定位：GitHub Windows 发布校验兼容性热修版，不包含新的业务功能。

## 修复内容

1. SHA-256 校验改用 .NET 流式实现，不再依赖可能被 GitHub `pwsh` 环境隐藏的 Windows PowerShell `Get-FileHash` 命令。
2. Authenticode 校验命令缺失时，从当前 PowerShell 安装目录显式加载官方安全模块。
3. 保留 `1.3.3` 的 Business Skills、Codex 运行时字节完整性和 NSIS 实装启动门禁。

## 根因

GitHub Actions 以 `pwsh` 启动 `pnpm`，包脚本再调用 Windows PowerShell。子进程继承的模块搜索路径无法发现 `Get-FileHash`，导致文件内容尚未开始比较就退出。安装包本身及 R2 `codex.exe` 的版本、大小、SHA-256 和 OpenAI 签名均无异常。

## 发布状态

- `releaseStatus: final-internal-accepted`
- 安装包：`半山商务工作台_1.3.4_x64-setup.exe`
- 大小：`131,407,867` bytes
- SHA-256：`5841a2c7f1efc203b5e184735217ff18ad890386bc855b4c393605a34d09ae91`
- 全新安装并同版本重装：通过；首次启动、重装后连续两次启动均保持运行 8 秒，内置 AI 凭据、Codex 许可文件、SQLite/Vault 保留、卸载和注册表恢复均通过。
- `1.3.3` 升级到 `1.3.4`：通过；升级前后版本识别正确，连续启动、内置 AI 凭据、SQLite/Vault 保留、卸载和注册表恢复均通过。
- 自动化检查：前端 `135` 项、Rust `605` 项、业务工具集成 `24` 项全部通过；另有 `4` 项依赖真实外部 Codex/FFmpeg 环境的测试按设计忽略。

## 安全说明

内部安装包继续包含当前 AI 与 R2 运行配置，且为未签名 NSIS 安装包。源码归档不得包含真实 `r2.config.json`、AI KEY 或 R2 KEY。公开 Git 历史曾包含旧 R2 凭据；成果保全完成后应单独轮换并收窄权限，再重新发布安装包。
