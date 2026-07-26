# 半山 AIGC Desktop 0.6.0

## 交付内容

- 需求访谈进入可用状态：按项目维护一份 Requirement Brief，把市场到编导的访谈答案整理为目标、受众、核心信息、交付物、渠道、风格、必含项、限制、验收标准、风险、截止时间、预算和参考案例等结构化内容。
- Requirement Brief 采用一项目一份的持久化模型，项目必须真实存在；项目外键、唯一约束和 `ON DELETE RESTRICT` 共同保护归属。Record、Event 和 Receipt 在同一 SQLite 事务中提交，支持幂等重放、revision CAS、deadline、trace 和应用重启恢复。
- 状态机覆盖 `interviewing`、`review` 和 `confirmed`。从访谈提交复核前必须通过后端完整度闸门；确认前还必须清空所有 `followUp`；已确认 Brief 禁止直接编辑，必须先重新打开。
- 固定题集版本为 `requirement-brief.v1`，共 14 题。每题支持 `unanswered`、`answered`、`followUp` 和 `notApplicable` 四种处理结果；前端只提交 `questionId`、答案和处理结果，题目文本、顺序与必填属性由 Rust 后端重建。
- 防篡改边界落在 Host：更新必须包含且只包含完整固定题集，未知、缺失或重复题号会被拒绝；即使持久化内容中的题目文本或必填标记被修改，读取时也会恢复服务端定义。文本、列表、时间戳和 UUID 同时执行长度、数量与格式校验。
- Requirement Brief 可通过稳定 `caseId` 引用案例素材库。Host 只接受全局案例或当前项目案例，拒绝不存在或属于其他项目的案例；协议、事件、Receipt 和响应不会暴露 Case 的本地存储路径。
- 权限面区分读取、写入和确认：模块注册器声明 `requirementBrief.read`、`requirementBrief.write`、`requirementBrief.confirm`、`project.read` 与 `case.read`；Host 对 list/replay 执行读权限检查，创建与内容更新走写权限，所有状态迁移统一走确认权限。
- Requirement Brief 使用独立连续事件流，提供 `requirementBrief.created`、`requirementBrief.updated` 和 `requirementBrief.statusChanged`。Client SDK 启动时先订阅并缓冲实时事件，再分页重放本地 Ledger；非连续事件在缺口闭合前不发布，实时缺口最多自动恢复三次，失败后可通过手动刷新重新收敛。
- 新增“需求访谈”工作区：项目栏显示未开始、访谈中、待确认和已确认状态；主工作区提供固定问题导航、处理结果、结构化摘要、完整度与待追问闸门、Case 勾选引用、保存、提交复核、退回补访、确认和重新打开操作，并显示 revision、更新时间与确认人。
- Requirement Brief 草稿按项目隔离。切换项目、刷新或保存期间继续输入时，迟到响应不会覆盖其他项目或新的未保存内容；确认态和请求执行期间在 UI 中只读。发现远端 revision 前进时，界面进入显式冲突态，由用户选择保留本地内容或载入最新版本。
- 项目建立 Requirement Brief 后，项目页旧 Brief 编辑器退出写入权威并引导到需求访谈；原有 `Project.brief` 数据不删除，继续作为创建 Requirement Brief 时的初始化来源和兼容回退。创建前若当前旧 Brief 仍有未保存编辑，客户端会先完成 Project Brief 持久化，再建立 Requirement Brief。
- 新建拍摄 Execution Brief 时，只有 `confirmed` Requirement Brief 才作为预填权威，映射客户目标、风格关键词、必含项、风险和限制；Requirement Brief 尚未确认或不存在时回退到旧 `Project.brief`。已经存在的 Execution Brief 不会被后续需求变更自动覆盖。
- Tauri Host、Client SDK、Desktop/Web HostAdapter 与 Rust/TypeScript 生成类型已接入统一协议面；React 页面不直接调用 Tauri IPC 或 SQLite。

## 协议 1.3

协议版本从 `1.2` 升级为 `1.3`。

新增命令：

- `requirementBrief.create`
- `requirementBrief.update`
- `requirementBrief.changeStatus`

新增事件：

- `requirementBrief.created`
- `requirementBrief.updated`
- `requirementBrief.statusChanged`

新增状态：

- `interviewing`
- `review`
- `confirmed`

新增答案处理结果：

- `unanswered`
- `answered`
- `followUp`
- `notApplicable`

新增记录包含 `questionSetVersion`、固定问题答案、结构化内容、状态、`confirmedAt`、`confirmedBy`、revision 和审计时间；新增事件通道为 `bsaigc://requirement-brief-event`。

协议兼容性说明：0.6.0 客户端、生成类型和 HostAdapter 统一使用 `1.3`。Host 对 0.5 已存在的 Project、Task、Asset、Case 和 Execution Brief 命令面保留受限 `1.2` 兼容，使升级前已提交命令及其 Receipt 可以精确重放；Requirement Brief 命令严格要求 `1.3`，其他版本一律拒绝。

## 状态机

```text
interviewing --完整度通过--> review --完整且无待追问--> confirmed
interviewing <--退回补访---- review <--重新打开-------- confirmed
```

- 完整度闸门要求 `objective`、`audience`、`keyMessage`、`deliverables`、`channels` 和 `acceptanceCriteria` 非空，并要求所有必答题不再处于 `unanswered`。
- `review` 状态仍可编辑，但每次保存都必须继续满足完整度闸门。
- `review -> confirmed` 会写入确认时间和当前操作者；`confirmed -> review` 会清除确认信息。
- `confirmed -> interviewing` 等跨级迁移会被拒绝；确认态内容不能直接更新。

## 验证

- 前端 Vitest：65 passed。
- Rust：225 passed，3 ignored。
- 3 个真实环境集成测试已单独执行并全部通过；它们不计入上述 225 个 Rust 常规测试。
- Requirement Brief 后端测试覆盖项目预填、固定题集重建、防篡改、一项目一份、文件数据库重开恢复、状态机、完整度与待追问闸门、确认人、Case 归属与不存在拒绝、路径信息隔离、幂等冲突、deadline、event/receipt 事务回滚、revision CAS、项目上下文和输入边界。
- Client SDK 测试覆盖启动订阅缓冲、分页重放、连续游标、缺口不提前发布、三次恢复上限、手动刷新收敛、事件去重、revision 过滤和 typed command 构造。
- UI 状态测试覆盖项目草稿隔离、干净草稿随远端 revision 刷新、保存期间的新输入保护，以及复核与确认闸门计算。

## 产物

安装器：

```text
src-tauri\target\release\bundle\nsis\半山AIGC Desktop_0.6.0_x64-setup.exe
Size: 45923737 bytes
SHA256: ED7CBA4A5289E7B7F56AAE86F0EEE1DDFE25ED9DF63F3656D5DDFD2823029560
```

Release executable：

```text
src-tauri\target\release\bsaigc_desktop.exe
Size: 13755904 bytes
SHA256: 0B3D8476D8B03BAE3704CB13AC005564E6A78559184152E6E0B11848CB1698C0
ProductVersion: 0.6.0
```

NSIS 安装后的 executable：

```text
%LOCALAPPDATA%\半山AIGC Desktop\bsaigc_desktop.exe
Size: 13755904 bytes
SHA256: EBE41ED139405DB8DCA2111FD9C06FC61B84C3486A33FB32F190D336B3409CD7
ProductVersion: 0.6.0
```

## 升级与数据兼容

- 0.6.0 在现有 SQLite 中增量创建 `requirement_briefs`、`requirement_brief_events` 和 `requirement_brief_command_receipts`，不替换 Project、Task、Asset、Case、Execution Brief、Memory 或 Brain 数据表。
- 从 0.5.0 升级后，既有项目不会自动生成 Requirement Brief；项目页继续使用原有 `Project.brief`，直到用户主动建立需求访谈。
- 建立 Requirement Brief 时会从当时的 `Project.brief` 复制可映射字段作为初始值。两套记录不做双向同步；建立后应在需求访谈中继续维护结构化需求。
- Requirement Brief 的项目外键采用 `ON DELETE RESTRICT`；存在 Requirement Brief 的项目不能被级联删除。
- Case 引用保存在 Requirement Brief 内容中的稳定 UUID 列表，不迁移、复制或改写原 Case 与 Asset 数据。
- 协议从 `1.2` 升级到 `1.3`。新客户端、生成类型、事件订阅和适配器成套使用 `1.3`；Host 仅在升级前已存在的五类命令面保留 `1.2` 兼容，确保 0.5.0 的未确认响应可以通过原 command/idempotency key 重放。Requirement Brief 不接受 `1.2`。
- 已有 Execution Brief 保持原内容和 revision；升级或后续确认 Requirement Brief 不会触发自动重写。只有之后新建的 Execution Brief 才按新的权威与回退规则预填。

## 已知边界

- 当前安装包未做 Windows 代码签名。
- Requirement Brief 当前为本地单机能力，一项目仅允许一份；列表为本地全量读取，尚未提供服务端分页、搜索、历史版本浏览、导出、多人协作或 Cloud 同步。
- `requirement-brief.v1` 是内置固定题集，当前不支持项目级自定义问题、题库模板或运行时扩展；未来调整题集必须配套显式版本与数据迁移。
- `confirmed` 表示当前结构化内容通过完整度与待追问闸门，并记录本地操作者，不等同于客户电子签署、合同审批、法务确认或外部资源落实。
- Desktop 0.6 使用单机单操作者信任模型，读、写和确认权限目前都在本地授权边界内放行，确认也不触发额外 Approval。未来 Cloud Host 必须从服务端会话派生身份，并在 list、replay、command 和 event delivery 上统一执行 account/project 资源权限，不能信任客户端自报上下文。
- Requirement Brief 对 Case 仅保存稳定 ID 引用，不冻结案例内容、资产版本或预览快照，也不形成不可变的客户确认证据包。
- 旧 `Project.brief` 仍保留为兼容数据，但 Requirement Brief 建立后 UI 不再提供旧 Brief 的并行编辑；当前也不会把已确认 Requirement Brief 反写到 `Project.brief`。
- Requirement Brief 只为新建 Execution Brief 提供预填，不自动推进项目阶段、不改变 Execution Brief 状态，也不验证拍摄场地、人员、器材、预算或交付资源已实际落实。
- `WebHostAdapter` 仅冻结协议，不提供网页版业务执行或持久化。
