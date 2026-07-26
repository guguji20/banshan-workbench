# 半山 AIGC Desktop 0.7.0

## 发布定位

0.7.0 新增商务单据中心的 headless 业务闭环：

```text
需求 -> 项目主数据 -> 报价 -> 合同 -> 请款 -> 验收 -> 回款
```

本版本交付 Rust Domain/Host、SQLite、内置文档 renderer、typed Tauri IPC 和 Client SDK，不新增 React 页面、导航或交互。当前能力可由 HostAdapter/Client SDK 调用；其中 `businessWorkspace.list` 只能走桌面 HostAdapter 映射的 typed Tauri IPC，不能把它描述成已经上线的商务 UI 或 Agent/Codex tool。

## 交付内容

- 一项目一份 `BusinessWorkspaceRecord`。创建工作区时验证 Project 真实存在，并从 Project 名称、客户名称和当时已确认的 Requirement Brief 预填项目标题、客户、交付摘要、验收条件、截止时间、风险/限制及初始服务明细；建立后不与 Project 或 Requirement Brief 隐式双向同步。
- 商务主数据覆盖项目编码与周期、客户/供应方资料、供应方银行账户、币种、默认税率、交付摘要、付款条款、验收条款、备注和报价明细。金额使用整数分，数量使用千分单位；Host 校验范围并按数量和单价计算明细金额，不接受客户端自报的计算结果。
- 支持 `quote`、`contract`、`paymentRequest`、`acceptance` 四类单据。创建单据时冻结完整 `BusinessDocumentSnapshot` 和来源 workspace revision；`paymentRequest` 必须绑定本工作区一笔 `planned/requested` Payment，并把付款记录冻结进快照。后续更新主数据或付款不会静默改写历史单据，需要变化时创建新的单据版本。
- 单据状态覆盖 `draft`、`inReview`、`approved`、`generated`、`voided`。批准时记录操作者与时间；只有 `approved` 可生成，`generated` 只能由生成命令写入。
- 内置 renderer 生成真实 OOXML 包：报价固定输出 XLSX，合同、请款和验收固定输出 DOCX；类型/格式不匹配会在产生副作用前拒绝。
- 生成文件只在 Vault 范围内 staging，staging 根、子目录和锁文件拒绝 symlink/Windows reparse point；随后由 Asset Service 原子入库并绑定项目。协议只返回稳定 `assetId`，单据记录 `outputAssetId`、格式和生成时间，不暴露绝对路径。
- 回款记录支持 `planned`、`requested`、`received`、`canceled`，保存应收/请款/到账金额、到期或发生时间、外部参考和备注；只有 `received` 可以填写实际发生时间。进入 `requested` 后，标签、金额和到期日被冻结；创建请款单也会冻结这三项。`received` 和 `canceled` 都是不可回退终态，金融字段被冻结，只允许补充备注。
- 每个 durable command 都带 `commandId`、`idempotencyKey`、`expectedRevision`、deadline 和 trace。request fingerprint 与 Receipt 保证同请求精确重放并拒绝 key/command 冲突；workspace revision CAS 拒绝陈旧写入。
- Business Workspace 使用独立连续事件流。Client SDK 在启动时先订阅并缓冲实时事件，再执行 list 和分页 replay；遇到 sequence 缺口时从连续游标恢复，重启后可从 SQLite Record/Event/Receipt 重建投影。
- 0.6 数据原地升级，只增量建立商务表，不改写既有 Project、Requirement Brief、Execution Brief、Case、Asset、Task、Memory 或 Brain 数据。

## 协议 1.4

协议版本从 `1.3` 升级为 `1.4`。新增命令：

- `businessWorkspace.create`
- `businessWorkspace.updateProfile`
- `businessWorkspace.createDocument`
- `businessWorkspace.changeDocumentStatus`
- `businessWorkspace.generateDocument`
- `businessWorkspace.upsertPayment`
- `businessWorkspace.changeStatus`

新增事件：

- `businessWorkspace.created`
- `businessWorkspace.profileUpdated`
- `businessWorkspace.documentCreated`
- `businessWorkspace.documentStatusChanged`
- `businessWorkspace.documentGenerated`
- `businessWorkspace.paymentUpserted`
- `businessWorkspace.statusChanged`

新增事件通道为 `bsaigc://business-workspace-event`。0.7 客户端、生成类型、HostAdapter 和模块清单统一使用 `1.4`。

### 兼容表

| 命令面 | Host 接受版本 | 说明 |
|---|---|---|
| Project、Task、Asset、Case、Execution Brief | `1.2`、`1.3`、`1.4` | 保留升级前 Receipt 的精确重放 |
| Requirement Brief | `1.3`、`1.4` | 0.6 命令面不接受 `1.2` |
| Business Workspace | 仅 `1.4` | 0.7 新命令面，无旧协议兼容 |

兼容只针对表中既有命令面；未知版本和不属于该命令面的旧版本直接拒绝。

## 模块契约

模块 ID 为 `business.documentCenter`，当前注册为 headless `Degraded` 能力：

| 类别 | 声明 |
|---|---|
| `commands` | `businessWorkspace.create`、`businessWorkspace.updateProfile`、`businessWorkspace.createDocument`、`businessWorkspace.changeDocumentStatus`、`businessWorkspace.generateDocument`、`businessWorkspace.upsertPayment`、`businessWorkspace.changeStatus` |
| `events` | `businessWorkspace.created`、`businessWorkspace.profileUpdated`、`businessWorkspace.documentCreated`、`businessWorkspace.documentStatusChanged`、`businessWorkspace.documentGenerated`、`businessWorkspace.paymentUpserted`、`businessWorkspace.statusChanged` |
| `permissions` | `businessWorkspace.read`、`businessWorkspace.write`、`businessWorkspace.approve`、`businessWorkspace.generate`、`asset.read`、`asset.import`、`project.read`、`requirementBrief.read` |
| `tools` | `businessWorkspace.list` |
| `storage` | `sqlite.business_workspaces`、`sqlite.business_documents`、`sqlite.business_payments`、`sqlite.business_workspace_events`、`sqlite.business_workspace_command_receipts`、`sqlite.assets`、`sqlite.asset_origins`、`vault.originals` |

`ModuleManifest.tools` 只是面向未来 Tool/MCP Registry 的声明性能力名录。能力名出现在该字段中，不代表工具已经注册、可被发现或可由 Agent/Codex 执行。0.7 尚未接入 Brain dynamic tool dispatch；`businessWorkspace.list` 的实际调用入口仍是 HostAdapter/typed Tauri IPC。

Desktop 0.7 仍是本地单操作者信任模型。list/replay 使用 `businessWorkspace.read`，创建/主数据/单据/回款/工作区状态使用 `businessWorkspace.write`，单据状态迁移使用 `businessWorkspace.approve`，生成使用 `businessWorkspace.generate`；当前这些操作按本地可逆写入授权，不触发额外 Approval。

## 存储与恢复

- `business_workspaces` 以 Project 外键和唯一约束保证一项目一工作区；`requirement_brief_id` 只记录创建时采用的已确认需求来源。
- `business_documents` 保存单据序号、唯一单号、`templateKey`、冻结快照、状态、批准信息和 Vault Asset 外键；请款快照包含所选 Payment，生成资产只能关联一份单据。
- `business_payments` 保存整数分金额和回款生命周期；已到账记录由数据库约束要求发生时间，并由 Host 状态机阻止回退和普通改写。
- `asset_origins` 为既有资产补记 `user` 来源，并为新生成文件写入不可由文件名伪装的 `businessDocument` generation provenance；协调回收只处理明确标记且未关联的生成资产。
- `business_workspace_events` 提供全局连续 sequence；`business_workspace_command_receipts` 保存请求 fingerprint 和完整响应。聚合变更、Event 和 Receipt 在同一 SQLite 事务提交，任一写入失败会整体回滚。
- 文档生成采用 staging、Asset 入库、最终关联三段式流程。Asset Event、单据关联、Business Event 和 Receipt 在最终事务中共同提交；最终提交失败、CAS 竞争或同命令竞态时清理本次未采用的资产。lease 防止并发回收误删活跃生成，启动阶段与后续商务命令都会重试中断 staging 和未关联生成资产的清理。
- 通用 Asset 导入在最终文件落盘前写后端私有恢复 intent。intent 不包含绝对路径、source token 或凭证；启动协调先处理 Asset intent，再处理 `businessDocument` provenance。完全匹配的已提交 Asset 保留原件，明确未提交的候选文件回收，数据库状态无法确认时保留 intent 和文件等待后续重试。
- 已提交 Receipt 的查询先于 deadline 和外部文件依赖，因此应用重启或原 staging 消失后，同一命令仍返回持久化响应；Client SDK 可通过 list/replay 恢复连续投影。

## 文档与模板

| 单据 | 输出 | 生成条件 |
|---|---|---|
| 报价 | XLSX | 单据已批准，快照含项目、客户、供应方、币种和至少一条报价明细 |
| 合同 | DOCX | 单据已批准，格式必须为 DOCX |
| 请款 | DOCX | 单据已批准，快照含供应方开户行与银行账户、付款条款和已绑定的正金额 Payment |
| 验收 | DOCX | 单据已批准，快照含交付摘要和验收条款 |

Host 已实现按单据类型区分的批准闸门；缺少周期、条款、银行、交付或付款快照时不会生成半成品。

字段目录和稳定模板键见 [BUSINESS_TEMPLATE_FIELDS.md](BUSINESS_TEMPLATE_FIELDS.md)。当前内置 renderer 仅用于验证闭环，真实公司 DOCX/XLSX 模板尚未接入，也不代表最终视觉、法务或财务格式。后续只替换 `template/renderer adapter` 并按 `templateKey` 版本化；Workspace、Document Snapshot、Payment、Command、Event、Receipt、Vault `assetId` 和 Client SDK 协议保持不变。

## 验证

- 前端 Vitest：11 个测试文件，74 passed。
- TypeScript：`tsc --noEmit` 通过。
- Rust：272 passed，3 ignored，0 failed。3 项 ignored 为需要真实 Codex CLI 或已 bootstrap FFmpeg runtime 的环境集成测试。
- 商务后端覆盖完整闭环、请款 Payment 快照、本期请款金额唯一来源、requested 字段冻结、received/canceled 终态审计、协议/状态/格式副作用前闸门、Event/Receipt 失败整体回滚、Asset provenance、Asset/Business 双事件、symlink/reparse 防护、安全 lease 打开、活跃生成和重启协调清理。
- Client SDK 覆盖商务 typed command、启动同步、分页 replay、实时缓冲、连续投影和事件缺口恢复。

本次验证没有启动或点击 live app，也没有生成 0.7 安装器；发布产物大小、哈希与签名信息待正式构建后补充。

## 升级与数据兼容

- 0.7.0 在现有 SQLite 中增量创建 `business_workspaces`、`business_documents`、`business_payments`、`business_workspace_events`、`business_workspace_command_receipts` 和 `asset_origins`。既有 Asset 只补记 `user` provenance，不移动、不复制、不改名。
- 升级后不会为既有项目自动创建商务工作区。首次显式创建时才读取当时的 Project 和已确认 Requirement Brief 作为预填来源。
- 创建工作区后，Project、Requirement Brief 和商务主数据各自保持权威，不做隐式同步；创建单据后，工作区与单据快照也不做隐式同步。
- 生成结果作为普通项目级 Vault Document Asset 保存，既有 Asset 数据和文件不迁移、不复制、不改名。
- 协议升级不改变旧命令面的有界兼容：`1.2` 仅保留给原有五类命令面，Requirement Brief 接受 `1.3`/`1.4`，Business Workspace 仅接受 `1.4`。

## 已知边界

- 0.7 没有新增商务 UI；当前 DesktopShell 中没有可操作的商务单据中心页面。
- 真实公司报价、合同、请款和验收模板尚未接入；当前内置 OOXML 只证明生成、Vault 入库和恢复闭环。
- 不包含电子签章、发票、银行接口、CRM 自动写入、法务审批、客户电子确认或云端多人协作。
- 单据 `approved` 只表示本地后端状态和操作者记录，不等同于法务批准、客户签署、开票、付款或实际验收。
- Desktop 0.7 仍是单机单操作者信任模型。未来 Cloud Host 必须从服务端会话派生身份，并在 list、replay、command 和 event delivery 上执行同一 account/project 资源权限策略。
- 当前安装包未做 Windows 代码签名；0.7 安装器尚未在本次文档收口中构建。
