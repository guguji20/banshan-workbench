# 半山 AIGC Desktop 0.1.0

构建日期：2026-07-19

## 交付范围

- Project 创建、结构化 Brief、生产阶段流转。
- SQLite Ledger、Command Receipt、revision CAS、Domain Event replay。
- BSAIGC Client SDK 与唯一 DesktopHostAdapter IPC 边界。
- 官方 Codex app-server 0.144.5 stdio 握手与状态面板。
- Windows x64 NSIS 安装包。

## 构建产物

| 产物 | 大小 | SHA-256 |
|---|---:|---|
| `bsaigc_desktop.exe` | 10,749,952 bytes | `c6a2e05fde568eda54fbd6ad2a7af6af7f9b6828a17483e06ce6022fcacc89dd` |
| `半山AIGC Desktop_0.1.0_x64-setup.exe` | 2,742,840 bytes | `6ebdaf4fe9bc3e3ab4827ee45447ea34e3d7d807e3ff92f92efaf1660791880c` |

安装包位置：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.1.0_x64-setup.exe
```

## 验证记录

- TypeScript 类型检查通过。
- Vitest：9 passed。
- Rust：19 passed，1 ignored（真实环境测试单独运行）。
- 官方 Codex app-server 真实握手测试通过。
- 真实桌面完成 Project 创建、Brief 保存、阶段变更。
- 关闭进程并重启后，从 SQLite 恢复 R3 与事件序列 #3。
- 1440×920 与 1120×720 窗口检查通过，无界面遮挡。
- CSP 生效后生产 release 启动和数据恢复通过。

## 发布限制

- 当前安装包未做企业代码签名。
- 默认发布目标为 NSIS；MSI/WiX 不作为 0.1.0 交付目标。
- 创意中心、无限画布、Task Engine、Provider、Native Media、Asset、Memory 和 Sync 尚未接入。
