# 半山 AIGC Desktop 0.4.0

## 交付内容

- 创意中心案例素材库进入可用状态：支持按标题、客户、标签、素材名、内容类型、表现形式、演员、AIGC 和质量等级筛选，并提供列表/网格视图。
- 案例只绑定已进入本地 Vault 的稳定 `assetId`，支持创建、编辑、revision CAS、幂等 receipt、事务事件和应用重启恢复。
- Rust `case_library` 服务与 Tauri Host、Client SDK、Desktop/Web HostAdapter 协议面完成接入；页面不直接调用 IPC 或 SQLite。
- BSAIGC 协议升级为 `1.1`，新增 Case Command、Event、Record 与分类枚举的 Rust/TypeScript 生成绑定。
- 案例事件投影使用连续 sequence 游标；实时事件出现缺口时自动从本地 Ledger 重放，不会越过丢失事件。
- 案例项目必须真实存在，并与来源资产项目严格一致；新数据库增加项目外键，案例事件禁止级联删除。
- 模块注册器只公布已经实现的 `case.list` 工具，不暴露未实现的 search/get 契约。

## 验证

- `pnpm verify`：通过。
- Vitest：32 passed。
- Rust：172 passed，3 ignored；3 个真实环境测试单独执行全部通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo fmt --check`：通过。
- 官方 Codex app-server 握手与远程线程列表：通过。
- 真实 Vault -> Durable Task Runner -> bundled ffprobe：通过。
- 真实源码桌面：从现有 AppData 创建案例、CAS 更新、关闭并重启后恢复 R2：通过。
- NSIS 静默升级：退出码 0；旧 Project、Asset、Brain 和 Case 数据全部保留，安装版系统状态 5/5。
- 安装版协议 `1.1` 写入：案例从 R2 更新到 R3，Ledger 新增 `case.updated` 和 protocol `1.1` receipt：通过。

## 产物

安装器：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.4.0_x64-setup.exe
Size: 45,756,429 bytes
SHA256: 5f5ffbc9e8d1192ace2576075adb9b99ee216b0d935629c0ad48bb9fa20cf338
```

Release executable：

```text
src-tauri\target\release\bsaigc_desktop.exe
Size: 13,121,536 bytes
SHA256: b19b8830153a12960b96b396b1855a34e5de462cd81c33502e57079cb9b1fa18
```

NSIS 安装后的 executable：

```text
%LOCALAPPDATA%\半山AIGC Desktop\bsaigc_desktop.exe
Size: 13,121,536 bytes
SHA256: 1a7c534f514381c5a95d6d57555218c0fc72efb17ff5f5684b51075ea3bd5fe4
ProductVersion: 0.4.0
```

## 已知边界

- 当前安装包未做 Windows 代码签名。
- 案例列表当前为本地全量读取；服务端分页搜索与大规模案例库压力测试留在后续版本。
- Desktop 0.4 使用单机单操作者信任模型。未来 Cloud Host 必须从服务端会话派生身份并实施 account/project 资源权限，不能信任客户端自报上下文。
- Provider 生成链、Asset Preview/Artifact、创意中心后续工具、无限画布、团队同步和 Cloud Host 尚未接入。
- `WebHostAdapter` 仅冻结协议，不提供网页版业务执行。
