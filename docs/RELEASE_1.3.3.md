# 半山商务工作台 1.3.3 更新说明

- 版本：`1.3.3`
- 日期：`2026-07-28`
- 定位：Windows 干净构建运行时完整性热修版，不包含新的业务功能。

## 修复内容

1. 将 OpenAI Codex Windows sandbox setup helper 明确纳入 Git，防止通用 `*-setup.exe` 忽略规则让 GitHub 干净构建缺少运行时文件。
2. 将 Codex `codex-package.json` 和 `manifest.json` 标记为字节完整性文件，禁止 Git 在不同操作系统上转换换行符。
3. 保留 `1.3.2` 已加入的 Business Skills LF 规范化、完整性校验和 GitHub 上传前 NSIS 安装启动验收。

## 发布门

GitHub Windows 流水线必须依次通过：

1. Business Skills 字节数和 SHA-256；
2. Codex 运行时文件齐全、版本、大小、SHA-256 与 Authenticode；
3. NSIS 初装、首次启动、内置 AI 凭据、重装、连续重启、SQLite/Vault 保留、卸载和注册表恢复；
4. 通过后才允许上传公开 R2 和 GitHub Artifact。

## 本地发布验收

- 安装包：`半山商务工作台_1.3.3_x64-setup.exe`
- 大小：`131,418,189` bytes
- SHA-256：`c72692fe8fc13368575d1936cc0f213c10cbde3b1c69b8b93b62db7cfcf59da0`
- 全新安装并同版本重装：通过；首次启动、重装后连续两次启动均保持运行 8 秒，内置 AI 凭据、Codex 许可文件、SQLite/Vault 保留、卸载和注册表恢复均通过。
- `1.3.2` 升级到 `1.3.3`：通过；升级前后版本识别正确，连续启动、内置 AI 凭据、SQLite/Vault 保留、卸载和注册表恢复均通过。
- 自动化检查：前端 `135` 项、Rust `605` 项、业务工具集成 `24` 项全部通过；另有 `4` 项依赖真实外部 Codex 环境的测试按设计忽略。

## 安全说明

内部安装包继续包含当前 AI 与 R2 运行配置。源码归档不得包含真实 `r2.config.json`、AI KEY 或 R2 KEY。公开 Git 历史曾包含旧 R2 凭据；成果保全完成后应单独轮换并收窄权限，再重新发布安装包。
