# 半山 AIGC Desktop 0.5.0

## 交付内容

- 拍摄执行中心进入可用状态：按项目维护一页拍摄执行单，项目列表直接展示“未创建”“草稿”或“可执行”状态。
- 执行单覆盖拍摄时间、客户目标、画面风格、主镜头、次镜头、必拍镜头、可替代镜头、风险点、等待时间利用、器材与现场备注，以及拍后素材亮点。
- 支持创建、编辑、保存、确认可执行、退回草稿和手动刷新；新执行单从所选项目预填基础上下文，并显示 revision 与最近更新时间。
- Rust 后端负责 Ready 完整度闸门。执行单只有在拍摄时间、客户目标、画面风格、主镜头、必拍镜头和风险点全部填写后才能进入 `ready`；已进入 `ready` 的执行单也不能通过后续编辑破坏这些必填项。
- Execution Brief 采用一项目一份的持久化模型，项目必须真实存在；项目外键、唯一约束和禁止级联删除共同保护执行单归属。
- 创建、更新和状态变更均经过 typed command、`commandId`、幂等键、deadline、trace、项目上下文与 revision CAS；Record、Event 和 Receipt 在同一 SQLite 事务中提交，可在应用重启后恢复。
- Execution Brief 独立事件流提供 `executionBrief.created`、`executionBrief.updated` 和 `executionBrief.statusChanged`。Client SDK 在启动阶段先订阅并缓冲实时事件，再分页重放本地 Ledger；检测到 sequence 缺口时从连续游标自动恢复。
- 非连续 Execution Brief 事件会先进入 SDK 缓冲区，只有游标连续后才更新可见记录；缺口恢复失败会自动重试三次，并可通过手动刷新再次收敛。
- 执行单草稿按项目隔离。切换项目或保存期间继续输入时，迟到的命令响应不会覆盖其他项目或新的未保存内容；版本尾标直接读取统一发布元数据。
- Tauri Host、Client SDK、Desktop/Web HostAdapter 与 Rust/TypeScript 生成类型已接入统一协议面；React 页面不直接调用 Tauri IPC 或 SQLite。
- BSAIGC 协议升级为 `1.2`，新增 Execution Brief Command、Response、Record、Status、Content、Event 及事件通道；模块注册器声明命令、事件、权限、`executionBrief.list` 工具和 SQLite 存储所有权。

## 协议 1.2

新增命令：

- `executionBrief.create`
- `executionBrief.update`
- `executionBrief.changeStatus`

新增事件：

- `executionBrief.created`
- `executionBrief.updated`
- `executionBrief.statusChanged`

新增状态：

- `draft`
- `ready`

协议兼容性说明：Desktop Host 对 Execution Brief 命令严格校验 `protocolVersion`；0.5.0 客户端应使用 `1.2`，旧协议版本不能调用新增命令。

## 验证

- Vitest：45 passed。
- Rust：194 passed，3 ignored。
- 3 个真实环境集成测试已单独执行并全部通过：官方 Codex app-server 握手、官方 Codex 远程线程列表、真实 Vault -> Durable Task Runner -> bundled ffprobe。它们不计入上述 194 个常规测试。
- Execution Brief 测试覆盖创建、更新、Ready 状态切换、幂等重放、文件数据库关闭重开、已提交命令过期重放、后端完整度闸门、event/receipt 写入失败事务回滚、revision CAS、项目上下文、缺失/重复项目拒绝、事件游标与内容边界。
- Client SDK 测试覆盖启动期间的事件缓冲与分页重放、非连续事件不提前发布、实时 sequence 缺口失败重试与刷新收敛、严格正数 revision，以及带项目归属和并发 revision 的命令构造。
- UI 状态测试覆盖按项目隔离草稿、干净草稿随远端 revision 刷新，以及保存期间的新输入不被迟到响应覆盖。

## 产物

安装器：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.5.0_x64-setup.exe
Size: 45833334 bytes
SHA256: ed3a45da134212655eef1a27bfef690eab4d93b115bd2800bd1df5d8996353a8
```

Release executable：

```text
src-tauri\target\release\bsaigc_desktop.exe
Size: 13391360 bytes
SHA256: d807f9e42a9e9e6a1148774c6569c4d4daa61d48838ed82629d7d36d1d705086
ProductVersion: 0.5.0
```

NSIS 安装后的 executable：

```text
%LOCALAPPDATA%\半山AIGC Desktop\bsaigc_desktop.exe
Size: 13391360 bytes
SHA256: d397dfce741067bca02693b829946e5e39f0cb9e3987c0469b24bf2bfa261b03
ProductVersion: 0.5.0
```

## 升级与数据兼容

- 0.5.0 在现有 SQLite 中增量创建 `execution_briefs`、`execution_brief_events` 和 `execution_brief_command_receipts`，不替换 Project、Task、Asset、Case、Memory 或 Brain 数据表。
- 从 0.4.0 升级后，既有项目默认显示为“未创建”，只有用户主动创建时才生成对应执行单。
- Execution Brief 的项目外键采用 `ON DELETE RESTRICT`；存在执行单的项目不能被级联删除。
- 协议从 `1.1` 升级到 `1.2`。需要与 Host 交互的客户端、生成类型和适配器必须成套升级。

## 已知边界

- 当前安装包未做 Windows 代码签名。
- Execution Brief 当前为本地单机能力，一项目仅允许一份执行单；列表为本地全量读取，尚未提供服务端分页、搜索、多人协作、历史版本浏览或导出。
- `ready` 仅代表六项拍前必填内容通过后端完整度校验，不代表场地、人员、器材、授权或外部资源已经实际确认。
- Desktop 0.5 使用单机单操作者信任模型。未来 Cloud Host 必须从服务端会话派生身份并实施 account/project 资源权限，不能信任客户端自报上下文。
- Provider 生成链、Asset Preview/Artifact、创意中心后续工具、无限画布、研究/合同工具、诊断上传、团队同步和 Cloud Host 尚未接入。
- `WebHostAdapter` 仅冻结协议，不提供网页版业务执行或持久化。
