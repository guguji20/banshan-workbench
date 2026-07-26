# 半山 AIGC Desktop 0.3.0

## 交付内容

- System Brain 主聊天接入官方 Codex app-server：线程、模型、Turn、流式回复、中断和重启恢复。
- Brain IPC 请求、结果和健康状态进入统一 Rust/TypeScript 生成协议；WebHostAdapter 保留同构接口。
- Durable Task Runner：固定并发、按已注册 kind 原子 claim、DAG、取消、panic 隔离、生命周期事件和安全关闭。
- Native Media Engine：结构化 probe、缩略图、音轨提取、deadline、取消、原子输出和 Vault 派生入库。
- 三个媒体任务 handler：`media.probe`、`media.thumbnail`、`media.extractAudio`；wire 输入只接受 `assetId` 和结构化选项。
- 安装包携带固定 SHA-256 的 FFmpeg LGPL shared runtime；源码可通过 `scripts/bootstrap-ffmpeg.ps1` 重建。
- Native Media 与 Brain 健康状态进入桌面系统状态栏。

## 验证

- `pnpm verify`：通过。
- Vitest：21 passed。
- Rust：153 passed，3 ignored；三个真实环境测试单独执行通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo fmt --check`：通过。
- 官方 Codex `initialize -> initialized -> thread/list -> shutdown`：通过。
- 真实桌面 Brain：创建线程、完成 `BRAIN_OK` Turn、应用重启后恢复：通过。
- 真实 Vault + Task Runner + bundled ffprobe probe：通过。
- NSIS 静默安装、安装后启动、AppData 保留、FFmpeg/ffprobe 发现：通过。

## 产物

安装器：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.3.0_x64-setup.exe
Size: 45,703,164 bytes
SHA256: 692a8ce569fd3d59319a714bfe281d4dc58002fb781247bf8f6bae53ef1179af
```

Release executable：

```text
src-tauri\target\release\bsaigc_desktop.exe
Size: 12,817,408 bytes
SHA256: 83acb944155dc17757f186ec1a02ce9b77ec42ef8a57999dc4d9dd85605a880c
```

FFmpeg runtime archive pin：

```text
ffmpeg-master-latest-win64-lgpl-shared.zip
SHA256: b3d3fb6928e5c146aa1194195742c9e9b708be2ad4f9db53a6bca79413c17bb6
Runtime: N-125658-g0869e710e6-20260718
```

## 已知边界

- 当前安装包未做 Windows 代码签名。
- Provider 生成链、Asset Preview/Artifact、创意中心、无限画布、团队同步和 Cloud Host 尚未接入。
- `WebHostAdapter` 仅冻结协议，不提供网页版业务执行。
