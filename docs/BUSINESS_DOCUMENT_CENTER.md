# 商务单据中心 0.9

## 目标

先闭环市场/客户经理最重复的一条链路：

```text
客户需求 -> 项目主数据 -> 报价 -> 合同 -> 请款 -> 验收 -> 回款
```

同一项目资料在项目工作区内只维护一次；重复客户可在新建工作区时显式复用历史客户/供应方基础资料，避免重新录入。每份商务文件创建时冻结主数据快照，后续修改主数据不会静默改变历史文件。生成结果先进入本地 Vault，只向 UI 返回稳定 `assetId`。

## 当前边界

0.9 继续只实现 Headless Domain、Rust Host、SQLite、内置文档引擎和 Client SDK，不冻结最终 React 页面、导航或交互设计。`businessWorkspace.list`、`businessWorkspace.prefillCandidates` 和 `businessWorkspace.previewPrefill` 当前只能经 Client SDK/HostAdapter 的桌面实现调用 typed Tauri IPC；`WebHostAdapter` 对这些能力仍返回 unavailable。它们出现在 `ModuleManifest.tools` 中仅表示面向未来 Tool/MCP Registry 的声明性能力名录，不代表已注册、可发现或可由 Agent/Codex 执行；Brain dynamic tool dispatch 尚未接入。

当前包含：

- 一项目一份商务工作区；
- 新建工作区可显式选择同客户历史工作区，一次性复制主数据白名单并保留来源审计；
- 客户、供应方、项目周期、税率、银行、付款和验收资料；
- 服务报价明细，金额由 Host 按整数分和千分数量计算；
- 报价、合同、请款、验收四类文档；
- 草稿、复核、批准、已生成、作废状态；
- 回款计划、请款和到账记录；
- 内置标准 DOCX/XLSX 模板；
- revision CAS、幂等 Receipt、deadline、连续 Event 和重启恢复；
- 从 0.8 数据原地升级：新增 `customer_name_key`、`customer_legal_name_key` 及两个候选查询索引；migration 只回填查询键，不改变 Workspace profile、来源审计、单据快照、付款或 Vault 资产。

当前不包含：

- 电子签章、发票和银行接口；
- 外部 CRM 自动写入；
- 法务审批和客户电子确认；
- 公司真实模板的视觉还原；
- 最终产品 UI；
- 云端多人协作。

## 数据权威

`BusinessWorkspaceRecord` 是当前项目商务主数据权威。它从目标 Project 和已确认 Requirement Brief 初始化；创建时还可显式传入 `prefillSourceWorkspaceId`，从同客户历史 Workspace 一次性复制以下白名单：

- 客户法定名称、税号、地址、联系人、电话、邮箱；
- 供应方法定名称、税号、地址、联系人、电话；
- 供应方开户行和银行账号；
- 币种和默认税率。

目标 Project 的项目名称/客户名称和已确认 Requirement Brief 的目标、交付、截止时间、验收与约束始终优先。项目编码、服务周期、交付说明、付款/验收条款、备注、报价明细、单据、回款、Workspace 状态和 revision 均不从历史 Workspace 复制。

来源 Workspace 必须存在，且目标 `Project.client_name` 与来源 `customer_name` 或 `customer_legal_name` 在删除全部 Unicode whitespace 并执行 Unicode lowercase 后匹配。复制和目标 Workspace 创建在同一事务完成；来源无效或客户不匹配时不留下 Workspace、Event 或 Receipt。创建成功后 `prefillSourceWorkspaceId` 随 list、Event replay 和重启恢复持久化，仅用于审计。来源与目标互不自动同步。

历史 Workspace 预填只影响新 Workspace 的初始主数据，不读取或修改任何既有 `BusinessDocumentRecord.snapshot`。`BusinessDocumentRecord.snapshot` 是单据历史权威。创建单据时复制工作区主数据和报价明细，并记录来源 workspace revision。请款单还必须选择一笔本工作区的 `planned/requested` Payment，并冻结该笔付款的 ID、金额、到期日和参考号。单据创建后，主数据或付款变化不会反写快照；需要新内容时创建新的单据版本。

## 历史候选查询

`list_business_workspace_prefill_candidates` 接收目标 `projectId` 和可选 `limit`。Client SDK 省略 `limit` 时显式发送 `50`，Host 接受 `1..=100`。目标项目必须存在且尚未建立 Business Workspace。

候选按来源 `updatedAt DESC, workspaceId DESC` 排序。客户匹配对目标 `Project.client_name`、来源 `customer_name` 和 `customer_legal_name` 使用同一规则：删除全部 Unicode whitespace，再执行 Unicode lowercase。匹配类型为 `customerName`、`customerLegalName` 或 `both`。archived Workspace 不会被静默排除，调用方可以根据返回状态决定是否展示或警示。

候选响应只返回：

- 来源 Workspace/Project ID、来源项目标题；
- 客户展示名、客户法定名和供应方法定名；
- 匹配类型、来源状态、revision、更新时间；
- 15 个白名单字段中哪些已填的字段名。

候选列表不返回税号、地址、联系人、电话、邮箱、开户行、银行账号、币种值或税率值。`customer_name_key` 与 `customer_legal_name_key` 只是 SQLite 查询索引，不是新的业务权威，也不得由 UI 或同步端当作客户主数据使用。

## 可解释差异预览

`preview_business_workspace_prefill` 接收一个目标 `projectId` 和一个来源 `workspaceId`。Host 会重新加载目标 Project、当时已确认的 Requirement Brief 和来源 Workspace，校验客户匹配后，固定按以下 15 个字段返回差异：

1. `customerLegalName`
2. `customerTaxId`
3. `customerAddress`
4. `customerContact`
5. `customerPhone`
6. `customerEmail`
7. `supplierLegalName`
8. `supplierTaxId`
9. `supplierAddress`
10. `supplierContact`
11. `supplierPhone`
12. `supplierBankName`
13. `supplierBankAccount`
14. `currency`
15. `defaultTaxRateBps`

每个字段都包含 `targetValue`、`sourceValue`、`resultValue` 和 `decision`。`decision` 只有四种：值相同为 `unchanged`，目标空而来源非空为 `filled`，双方非空且不同为 `replaced`，目标非空而来源空为 `cleared`。`resultValue` 表示按当前白名单创建时将采用的来源值。

preview 只是请求时点的解释，不签发 token，也不是 CAS。真正执行 `businessWorkspace.create` 时，Host 会再次读取来源、再次校验客户，并再次按固定白名单应用字段；因此来源在 preview 后发生变化时，创建结果以 create 事务实际读取的数据为准。未来商务 UI 只能消费后端候选和预览，不得自行实现客户匹配、字段复制或差异决策。

`BusinessPaymentRecord` 是回款状态权威。金额只使用整数分，不使用浮点数。状态按 `planned -> requested -> received` 推进，未到账记录可取消；只有 `received` 可以填写实际发生时间。进入 `requested` 后，标签、金额和到期日被冻结；创建请款单也会冻结这三项。`received` 和 `canceled` 都是终态，不能回退，金融字段不能由普通 upsert 改写，只允许补充备注。

## 文档规则

| 单据 | 默认格式 | 最低生成条件 |
|---|---|---|
| 报价 | XLSX | 项目、客户、供应方、币种、至少一条报价明细 |
| 合同 | DOCX | 报价条件 + 服务周期、付款条款、验收条款 |
| 请款 | DOCX | 客户、供应方、开户行与银行账户、付款条款，以及已选择并冻结的请款记录 |
| 验收 | DOCX | 客户、供应方、交付摘要和验收条款 |

只有 `approved` 单据可以生成。每次生成产生新的 Vault Document Asset，并把 `assetId`、格式和生成时间写回单据。Asset 使用内部 `businessDocument` provenance 标记，不能从展示文件名推断归属；成功提交时同时发布 Asset Event 和 Business Event。生成失败不能把单据错误标记为已生成，中断 staging 和未关联生成资产由 lease + 启动/命令协调恢复清理。

## 模板替换

内置模板只负责验证完整闭环。公司真实 DOCX/XLSX 到位后，通过稳定 `templateKey` 替换模板执行器；Workspace、Document Snapshot、Payment、Command、Event、Receipt 和 Client SDK 协议保持不变。

真实模板至少需要：

- 2-3 份报价 XLSX；
- 2-3 份合同 DOCX；
- 请款 DOCX；
- 验收 DOCX；
- 公司固定开票、银行和签章信息；
- 可接受的字段名与金额计算规则。

## 迁移约束

未来回套公司 BSAIGC 时优先迁移 Rust Domain、SQLite migration、document engine、generated protocol 和 Client SDK。Tauri IPC 只是一种 HostAdapter；最终 UI 可以重新设计，不需要复制当前 DesktopShell。
