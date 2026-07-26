# 半山 AIGC Desktop 0.8.0

## 发布定位

0.8.0 不新增商务 React 页面，而是在 0.7 的 headless 商务闭环上补齐“重复客户基础资料复用”：

```text
需求 -> 项目主数据 -> 报价 -> 合同 -> 请款 -> 验收 -> 回款
```

新建商务工作区时，调用方可显式选择同客户历史工作区，将稳定的客户、供应方、银行、币种和默认税率一次性复制到新工作区。该能力复用既有 Command、Event、Receipt、SQLite、Client SDK 和重启恢复体系，不建立来源与目标之间的长期同步关系。

## 交付内容

- `businessWorkspace.create` 新增 nullable `prefillSourceWorkspaceId`。未选择历史来源时行为与 0.7 一致；显式选择时，来源工作区必须真实存在且属于同一客户。
- 客户匹配会移除所有空白并忽略 ASCII 大小写。来源 `customer_name` 或 `customer_legal_name` 任一与目标 `Project.client_name` 匹配即可。
- 只复制以下白名单：
  - 客户法定名称、税号、地址、联系人、电话和邮箱；
  - 供应方法定名称、税号、地址、联系人和电话；
  - 供应方开户行和银行账号；
  - 币种和默认税率。
- 目标 Project 与当时已确认 Requirement Brief 始终优先。项目名称、客户展示名、项目编码、服务周期、交付说明、付款/验收条款、备注、报价明细、Documents、Payments、状态和 revision 不从历史工作区复制。
- 复制与新工作区创建处于同一 SQLite 事务。来源不存在或客户不匹配时，不留下 Workspace、Event 或 Receipt。
- 创建成功后，`prefillSourceWorkspaceId` 随 Workspace Record、created Event、list、replay 和应用重启恢复持久化，用于审计来源。
- 复制是创建时快照。来源后续修改不会自动更新目标，目标修改也不会反写来源；既有 `BusinessDocumentSnapshot` 永不受后续预填或主数据修改影响。

## 协议 1.5

BSAIGC 协议由 `1.4` 升级为 `1.5`。当前兼容矩阵：

| 命令面 | Host 接受版本 | 说明 |
|---|---|---|
| Project、Task、Asset、Case、Execution Brief | `1.2`、`1.3`、`1.4`、`1.5` | 保留升级前 Command/Receipt 精确重放 |
| Requirement Brief | `1.3`、`1.4`、`1.5` | 不接受更早协议 |
| Business Workspace | `1.4`、`1.5` | `1.5` 支持显式历史预填；`1.4` 保留旧 Receipt 精确重放 |

### 1.4 精确重放语义

- `1.4 businessWorkspace.create` 的 `prefillSourceWorkspaceId` 必须缺省或为 `null`。
- Host 对该请求按 0.7 旧载荷 `{ "projectId": "..." }` 计算 fingerprint，保证升级前已写入的 1.4 Receipt 能使用原 `commandId` 和 `idempotencyKey` 精确重放。
- `1.4` 请求携带非空来源 ID 时返回 `BUSINESS_PREFILL_PROTOCOL_UNSUPPORTED`，不会把 1.5 语义伪装成旧协议。
- 新客户端、HostAdapter、生成 TypeScript 和模块清单统一发送 `1.5`。
- `package.json`、Rust crate、Cargo lock 与 Tauri 应用版本统一升级为 `0.8.0`。

## SQLite 增量迁移

- 既有 `business_workspaces` 原地增加 nullable `prefill_source_workspace_id`。
- 旧 `business_workspace_command_receipts` 若仍限制 `protocol_version = '1.4'`，迁移会在事务中重建为允许 1.4/1.5 共存的约束并保留原数据。
- 不改写既有 Project、Requirement Brief、Execution Brief、Case、Asset、Brain、Business Document、Payment 或 Vault 文件。
- 不为既有项目自动创建工作区，也不会替既有工作区猜测历史来源。

## Client SDK

SDK 暴露兼容输入：

```ts
export type CreateBusinessWorkspaceInput = Omit<
  CreateBusinessWorkspacePayload,
  "prefillSourceWorkspaceId"
> & {
  readonly prefillSourceWorkspaceId?: string | null;
};
```

调用方可继续只传 `projectId`；SDK 会将缺省值归一化为 `null`。显式来源 ID 会通过纯 JSON Command 发送，Workspace 投影会保留该审计字段。UI 仍不直接调用 Tauri `invoke()`。

## 验证

截至 2026-07-19，本轮门禁覆盖：

- 协议生成测试：121 passed；
- 前端 Vitest：11 个测试文件，74 passed；
- Rust 默认全目标测试：284 passed，3 ignored，0 failed；
- 3 个 opt-in 真实集成测试显式运行并全部通过：官方 Codex app-server 握手、`thread/list` 和 bundled FFprobe Vault 探测；
- TypeScript 检查、生产构建、Rust format、Clippy `-D warnings` 与完整协议生成通过；
- 关键回归覆盖白名单复制、目标字段优先、客户匹配、事务回滚、来源审计、快照隔离、来源修改隔离、重启恢复、1.4 Receipt 精确重放及 1.4/1.5 Receipt 共存。

默认测试仍将 3 个真实运行时用例标记为 ignored，避免没有 Codex CLI/FFmpeg runtime 的普通开发环境误报；本次发布门禁已使用本机官方 Codex CLI 和已 bootstrap 的 bundled FFprobe 单独验证。

## 已知边界

- 0.8.0 没有新增商务 UI、导航、最终视觉或交互；能力仍通过 Rust Host、typed Tauri IPC 和 Client SDK headless 使用。
- 尚未接入公司真实报价 XLSX、合同 DOCX、请款 DOCX 和验收 DOCX；内置 OOXML renderer 只验证生成、Vault 入库、审计和恢复闭环。
- 历史来源必须由调用方显式选择；当前不提供自动候选、模糊客户合并、字段级选择或 UI 差异预览。
- 预填不是 CRM 同步。来源和目标创建后相互独立，不会持续同步或自动刷新。
- 不包含电子签章、发票、银行接口、CRM 自动写入、法务审批、客户电子确认或云端多人协作。
- `approved` 仅表示本地后端工作流状态，不等同于法务批准、客户签署、开票、付款或实际验收。
- 当前 Desktop 仍是单机单操作者信任模型；Cloud Host 的服务端身份、团队资源权限和 WebSocket 事件传输尚未实现。

## 升级结论

0.8.0 的增量保持在通用地基内：业务权威仍在 Rust/SQLite，文件权威仍在 Vault，前端仍只通过 Client SDK 消费纯 JSON Command/Event。未来补商务 UI 或回套公司 BSAIGC 时，可以直接复用本轮 Domain、migration、协议、SDK 和测试，不需要重写业务闭环。
