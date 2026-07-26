# BSAIGC Desktop 0.9.0

发布日期：2026-07-19

## 发布结论

0.9.0 在 0.8 的“显式选择同客户历史 Workspace 并一次性复制白名单”基础上，补齐两个 headless 读取能力：

1. 查询目标项目可用的同客户历史 Workspace 候选；
2. 对一个明确来源生成固定 15 字段的可解释预填差异预览。

本轮仍不增加商务 React 页面、导航、视觉或最终交互，也不接入公司真实 DOCX/XLSX 模板。业务权威继续位于 Rust/SQLite，前端只能经 Client SDK 与 HostAdapter 使用纯 JSON 协议。

0.9.0 是桌面应用版本升级。候选和预览属于 additive read contract，不写 durable Command Receipt/Event，不改变既有命令载荷、幂等、revision CAS 或重放语义，因此 BSAIGC durable command 协议继续保持 `1.5`。

## 新增读取协议

Rust `src-tauri/src/protocol.rs` 继续作为 source of truth，并生成以下 TypeScript 类型：

- `BusinessWorkspacePrefillField`
- `BusinessWorkspacePrefillMatchKind`
- `BusinessWorkspacePrefillDecision`
- `BusinessWorkspacePrefillCandidate`
- `ListBusinessWorkspacePrefillCandidatesRequest`
- `PreviewBusinessWorkspacePrefillRequest`
- `BusinessWorkspacePrefillChange`
- `BusinessWorkspacePrefillPreview`

新增 typed Tauri IPC：

- `list_business_workspace_prefill_candidates`
- `preview_business_workspace_prefill`

两者都复用 `businessWorkspace.read` 权限，不创建 Workspace，不写 Business Event，也不写 Command Receipt。

模块声明新增：

- `businessWorkspace.prefillCandidates`
- `businessWorkspace.previewPrefill`

原有 `businessWorkspace.list` 保留。三者当前只是 `ModuleManifest.tools` 中面向未来 Tool/MCP Registry 的声明性能力名，不代表 Brain dynamic tool dispatch 已经接入。

## 同客户候选查询

候选查询以目标 `projectId` 为入口。目标 Project 必须存在且尚未建立 Business Workspace。Client SDK 省略 `limit` 时发送 `50`；Host 接受 `1..=100`。

客户身份匹配统一使用：

1. 删除全部 Unicode whitespace；
2. 对剩余字符执行 Unicode lowercase。

来源 `customer_name` 或 `customer_legal_name` 任一匹配目标 `Project.client_name` 即可进入候选；同时匹配时返回 `both`。候选按 `updatedAt DESC, workspaceId DESC` 排序，archived Workspace 仍可进入候选并显式返回状态，不由后端静默隐藏。

候选响应只包含来源 Workspace/Project 元数据、客户/供应方法定展示信息、匹配类型、来源状态、revision、更新时间，以及固定白名单中已填的字段名。它不会返回客户税号、地址、联系人、电话、邮箱、供应方税号、开户行、银行账号、币种值或税率值。

## 可解释差异预览

调用方选择一个来源后，可请求单来源 preview。Host 会加载目标 Project、当时已确认的 Requirement Brief 和来源 Workspace，重新验证客户匹配，并固定返回以下 15 个字段：

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

每个字段都返回：

- `targetValue`：目标 Project/已确认 Requirement Brief 形成的初始值；
- `sourceValue`：所选来源 Workspace 当前值；
- `resultValue`：按当前固定白名单创建时会采用的值；
- `decision`：`unchanged`、`filled`、`replaced` 或 `cleared`。

preview 是请求时点说明，不是快照凭证，不签发 preview token，也不提供 CAS。真正执行 `businessWorkspace.create` 时，Host 会重新读取来源、重新校验客户，并重新应用固定白名单。来源在 preview 后被修改时，创建事务读取到的最新有效来源才是最终结果。

未来商务 UI 必须消费后端候选与 preview，不得在 React 内自行实现客户匹配、字段复制、差异决策或敏感值聚合。

## SQLite migration

`business_workspaces` 新增：

- `customer_name_key`
- `customer_legal_name_key`

新增两个 `(identity_key, updated_at DESC, id DESC)` 查询索引。migration 会从现有 Workspace profile 回填归一化查询键；只有期望查询键与现有值不同才执行 UPDATE，避免合法空法定名在每次启动时被重复改写。

这两个 key 只用于本地候选查询和索引，不是新的客户主数据权威。真实客户信息仍以目标 Project 和 `BusinessWorkspaceRecord.profile` 为准。migration 不修改既有来源审计、单据快照、付款、Receipt/Event 或 Vault 资产。

## UUID 与错误边界

两个读取入口都会 trim UUID，接受大写 UUID，并向后续服务返回 canonical UUID。畸形目标/来源 UUID 返回 `VALIDATION_FAILED`。

主要业务错误保持结构化：

- `PROJECT_NOT_FOUND`
- `BUSINESS_WORKSPACE_EXISTS`
- `BUSINESS_PREFILL_SOURCE_NOT_FOUND`
- `BUSINESS_PREFILL_CUSTOMER_MISMATCH`
- `VALIDATION_FAILED`

读取失败不会留下 Workspace、Event 或 Receipt。

## Client SDK

`HostAdapter`、`DesktopHostAdapter` 和 `BsaigcClient` 新增 typed 读取方法：

```ts
client.listBusinessWorkspacePrefillCandidates({
  targetProjectId,
  limit: 50,
});

client.previewBusinessWorkspacePrefill({
  targetProjectId,
  sourceWorkspaceId,
});
```

`DesktopHostAdapter` 只通过 `{ request }` 调用 typed Tauri IPC。`BsaigcClient` 省略 `limit` 时发送 `50`；调用方显式传入的 limit（包括 `null`）原样透传。preview 原样消费 Host 结果，前端不重复计算差异。

`WebHostAdapter` 仍是未来 HTTPS/WebSocket 迁移占位，对新增读取能力返回既有 unavailable，不引入双端维护成本。

## 兼容性

- 桌面应用、Rust crate 和 Tauri 配置版本统一为 `0.9.0`。
- BSAIGC durable command 协议仍为 `1.5`。
- 既有 `businessWorkspace.create`、1.4 Receipt 精确重放、1.5 显式来源预填、Event replay、revision CAS 与重启恢复语义不变。
- 0.9.0 不增加新的 durable command 或 event 类型。
- 现有 React 页面不直接调用 Tauri `invoke()`；新增能力只经 Client SDK/HostAdapter 暴露。

## 验证范围

本轮回归覆盖：

- 候选匹配 `customerName`、`customerLegalName` 和 `both`；
- Unicode whitespace 删除与 Unicode lowercase；
- archived 来源仍进入候选；
- 候选只返回元数据和已填字段名；
- 默认/显式 limit 及非法 limit；
- 固定 15 字段顺序与四类 preview decision；
- preview 来源不存在、客户不匹配、目标已有 Workspace；
- 大写 UUID canonical 化与畸形 UUID 拒绝；
- 所有读取错误路径不写 Workspace、Event 或 Receipt；
- identity key migration 与空法定名不重复改写；
- DesktopHostAdapter、WebHostAdapter、BsaigcClient 的 typed 边界。

发布门禁使用：

```powershell
pnpm protocol:generate
pnpm verify
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

默认 Rust 测试中的官方 Codex app-server handshake、官方 `thread/list` 和 bundled FFprobe Vault probe 仍为 ignored opt-in 测试；发布前需分别用 `--ignored --nocapture` 显式运行。

截至 2026-07-19，本轮实际门禁结果：

- 协议生成测试：129 passed；
- 前端 Vitest：11 个测试文件，77 passed；
- Rust 全目标默认测试：303 passed，3 ignored，0 failed；
- Cargo format、Clippy `-D warnings`、TypeScript check、生产 build 与 `git diff --check` 全部通过；
- bundled FFprobe Vault 真实探针显式运行通过；
- 两个官方 Codex 真实探针通过。当前 Windows Store PATH 项受执行 ACL 限制，因此测试使用受支持的 `BSAIGC_CODEX_BIN` 显式指向可执行的官方 Codex CLI；这不改变产品的候选发现或 Host 协议。

## 已知边界

- 0.9.0 没有新增商务 UI、导航、最终视觉或交互。
- 尚未接入公司真实报价 XLSX、合同 DOCX、请款 DOCX 和验收 DOCX。
- preview 不锁定来源，不承诺 preview 与稍后 create 读取到完全相同的来源 revision。
- 当前没有字段级勾选；create 始终应用完整 15 字段白名单。
- archived 来源仍由调用方决定是否允许用户选择。
- 不包含电子签章、发票、银行接口、CRM 自动写入、法务审批、客户电子确认或云端多人协作。
- 当前 Desktop 仍是单机单操作者信任模型；Cloud Host 的服务端身份、团队资源权限和 WebSocket 事件传输尚未实现。

## 升级结论

0.9.0 把“知道有哪些历史资料可复用”和“在创建前看清楚会改什么”收进统一 Rust/SQLite/Client SDK 地基。未来补商务 UI 或回套公司 BSAIGC 时，只需设计候选选择和差异展示，不需要把客户匹配、敏感数据边界、字段复制或最终创建规则搬进前端重写。