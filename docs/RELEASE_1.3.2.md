# 半山商务工作台 1.3.2 更新说明

- 版本：`1.3.2`
- 日期：`2026-07-28`
- 定位：Windows 云构建启动闪退热修版，不包含新的业务功能。

## 修复内容

1. 修复 GitHub Windows 构建环境将文本文件检出为 LF 后，内置 Business Skills 清单仍引用本地 CRLF 字节数和 SHA-256，导致应用首次启动以退出码 `101` 闪退的问题。
2. 增加 Business Skills 清单重建脚本，生成前统一使用 LF 并重新计算每个文件的字节数和 SHA-256。
3. 将 Business Skills 完整性检查加入常规质量门。
4. GitHub Windows 流水线在上传 R2 和 GitHub Artifact 前，必须完成初装、启动、内置凭据、重装、重启、数据保留、卸载和注册表恢复验收。
5. GitHub Artifact 同时保存安装包和本次验收摘要。

## 事故结论

`v1.3.1` 的本地安装包已通过验收，但同一提交在 GitHub Windows Runner 上因换行符差异生成了会闪退的云端安装包。`v1.3.2` 不覆盖旧标签，用新标签和新安装包替换公开更新入口。

## 本地验收结果

- 安装包：`半山商务工作台_1.3.2_x64-setup.exe`
- 大小：`131413604` 字节
- SHA-256：`d5d84cc87ca119938a3206e726ecb1ff1f8de135d9a707ce6e826637fc1b7d21`
- 全新安装与同版本重装：通过
- 从 `1.3.1` 覆盖升级到 `1.3.2`：通过
- 内置 AI 凭据探测、连续重启、SQLite/Vault 数据保留、卸载与注册表恢复：通过
- 验收记录：`.runtime/nsis-acceptance/release-1.3.2-20260728-local-final/acceptance-summary.json`
- 升级验收记录：`.runtime/nsis-acceptance/release-1.3.2-20260728-upgrade-from-1.3.1/acceptance-summary.json`

## 安全说明

内部安装包继续包含当前 AI 与 R2 运行配置。公开 Git 历史曾包含旧 R2 凭据；完成本次成果保全后，应单独轮换并收窄该凭据权限，再重新发布安装包。
