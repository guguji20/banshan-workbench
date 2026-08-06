# 业务工作台 1.0 候选版记录

> 自 2026-08-02 起，本文件仅作为历史 Windows 验收构建和升级回滚证据；不再代表可独立分发的内部 RC、Windows RC 或正式候选版。正式 1.0 必须在全部跨平台与真实商务门禁关闭后一次性发布。

- **事实日期**：2026-07-29
- **产品阶段**：历史 Windows 验收构建（非发行）
- **程序版本**：1.3.4

## 候选产物

- Installer SHA-256：`176e...f5b3`
- Installer 大小：`132211762 bytes`
- Authenticode：`NotSigned`
- 安全检查：未内置 API Key 或 R2 凭据

## 验证结果

- `pnpm release:verify`：36 files / 217 tests，通过
- Rust：818 passed / 13 ignored
- tools：24 passed
- NSIS same-package-reinstall：通过
- 冷启动 20 次：通过
- 卸载保留：通过
- Source snapshot：1526 files，0 sensitive findings

## 未完成门禁

- 真实跨版本升级验证
- 第二台 Windows 设备及独立用户验证
- macOS 签名、公证和启动验证
- Windows Authenticode 签名

## 日期说明

验收摘要文件时间显示 `2026-07-30`，属于宿主机时钟异常；本报告的权威事实日期仍为 **2026-07-29**。
