# 半山 AIGC Desktop 0.2.0

构建日期：2026-07-19

## 交付范围

- Durable Task Engine：DAG、优先级、attempt fencing、取消、审批重试、完整生命周期事件和重启恢复。
- Asset Vault：原生选择器、一次性 source token、流式 SHA-256、原子落盘、项目内去重、Event/Receipt。
- 任务中心和资产库桌面页面，统一 Client SDK 投影、启动竞态缓冲和事件重放。
- Global/Project/Thread Memory Service 与 revision CAS。
- Approval Ledger 和 Developer Diagnostic Outbox，诊断入库前脱敏。
- 官方 Codex app-server 持久 Runtime、Thread/Turn/Interrupt、本地 Brain Ledger 和轻量流式事件。
- 模块注册表及纯 JSON Desktop/Web HostAdapter 协议边界。

## 构建产物

| 产物 | 大小 | SHA-256 |
|---|---:|---|
| `bsaigc_desktop.exe` | 12,387,840 bytes | `03af21231681ff8ff2be6dcee1f41c3d78766b5b4daf4043f3f968103ef9b53f` |
| `半山AIGC Desktop_0.2.0_x64-setup.exe` | 3,104,395 bytes | `733c9f8cbdd045d3af91dd89590726929f6c3e379f13aa5f4a9aa166d7fdd2ba` |

安装包位置：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.2.0_x64-setup.exe
```

## 验证记录

- BSAIGC TypeScript 协议生成与一致性检查通过，导出 54 类协议绑定。
- Vitest：13 passed；TypeScript 类型检查和生产 WebView 构建通过。
- Rust：117 passed，2 ignored；两个 ignored 官方 Codex 真实测试均单独运行通过。
- `cargo clippy --all-targets -- -D warnings` 和 `cargo fmt --check` 通过。
- 官方 Codex app-server `initialize -> initialized -> thread/list -> shutdown` 长连接通过。
- 真实桌面 Task Center、Asset Vault、1440x920 布局验收通过。
- 原生选择器导入 90B 文本样本，Vault 与源文件 SHA-256 完全一致。
- Asset、Event、Receipt 同时落库，序列化数据不含绝对路径或 `sourceToken`。
- 关闭 debug 进程并重启后，项目 R3、事件 #3 和导入资产均从 AppData 恢复。
- 0.2.0 release 可执行文件已启动并读取同一份本地权威数据。

## 发布限制

- 当前安装包未做企业代码签名。
- Provider Runner、Native Media 预览/转码、Artifact、创意中心、无限画布和团队同步尚未接入。
- Brain Server Request 第一版采用保守中断并记录审批，不在客户端自动放行不可逆工具调用。
- 当前不实现网页版；`WebHostAdapter` 仅保留协议边界。
