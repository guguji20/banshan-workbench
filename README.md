# 华邦互娱商务系统

> 版本：`1.3.0`
> 发布日期：`2026-07-26`
> 形态：Windows 桌面版（Tauri + React + Rust）

华邦互娱商务系统是一套面向客户经理和商务岗位的本地优先桌面系统。它把客户资料、需求访谈、报价、合同审查、请款、验收、回款、台账和归档收敛到同一条可追踪、可恢复的业务链中，并以接近 Codex Desktop 的 Agent 工作方式减少重复查找、重复录入和反复确认。

## 1.2 商务闭环

```text
客户/项目
→ 需求访谈
→ 报价
→ 合同导入、解析、规则审查与 Codex 智能审查
→ Evidence 原文定位
→ 人工确认
→ 审查报告
→ 请款/付款
→ 验收
→ 到账/回款
→ 台账与归档
```

核心能力：

- **客户与项目**：独立维护客户主数据和项目归属，承接后续单据、合同、交付、发票和回款记录。
- **需求访谈**：结构化整理客户目标、交付范围、时间、预算、验收口径和待补信息。
- **报价与单据**：生成报价、合同、请款和验收文件；单据创建时冻结业务快照，避免后续资料变化污染历史记录。
- **合同审查**：导入 PDF/DOCX，执行确定性规则检查和 Codex 智能审查，保留风险等级、规则编码、Evidence 原文和人工决策。
- **报告闭环**：人工逐条确认后生成 HTML/DOCX 审查报告，结果先写入 Local Vault，再异步备份。
- **交付与签收**：维护交付里程碑、交付物版本、发送批次和客户签收证据。
- **发票与收付款**：记录发票开具、红冲、附件、付款计划、请款、到账和回款状态。
- **完整归档**：生成带 Manifest、SHA-256 和业务快照的 ZIP 归档包，归档和导出前再次校验 Local Vault 完整性。
- **可恢复执行**：任务、事件、版本和审批状态持久化；后台操作支持取消、重试、恢复和追踪。

## 本地权威与 R2 备份

- SQLite 是任务和业务记录的唯一权威。
- Local Vault 是合同原件、附件、报告和生成文件的唯一主存储权威。
- 所有导入与生成结果必须先成功写入 Local Vault，随后才向界面返回稳定 `assetId`。
- Cloudflare R2 只承担异步灾备和显式恢复，不承担业务数据库、任务成功判定或界面主读取来源。
- 断网、R2 未配置或备份失败不会阻断本地合同审查、报告生成和台账操作；失败项保留在 Durable Backup Outbox 中等待重试。
- 从 R2 恢复时必须校验对象大小、元数据和 SHA-256，校验通过后才原子写回 Local Vault。

## 官方 Codex app-server

系统直接复用官方 Codex app-server 作为唯一 Agent Runtime，不维护第二套自研 Codex Core。React 只通过 BSAIGC Client SDK 提交命令和消费事件；Codex 进程、工具调用、审批、任务恢复和长期记录均由 Rust Backend Host 管理。

### 无凭据规则降级

未配置 AI 凭据、网络不可用或智能审查调用失败时，合同审查会进入**规则降级模式**：

1. 已完成的确定性规则 Findings 和 Evidence 不丢失；
2. 审查会话进入待人工确认状态，而不是锁死为失败；
3. 用户仍可逐条确认风险并生成 HTML/DOCX 报告；
4. 在尚未产生人工决策时，可以稍后重试智能审查；
5. 已产生人工决策后，不允许智能重跑覆盖人工结果。

## 架构边界

```text
React UI
→ BSAIGC Client SDK
→ DesktopHostAdapter
→ Typed Tauri IPC
→ Rust Backend Host
→ SQLite / Local Vault / Official Codex app-server / R2 Backup Adapter
```

硬边界：

- React 不直接调用 Tauri `invoke`、Provider、Codex、R2 或本地绝对路径。
- UI 只接收轻量事件、进度、预览描述和稳定 ID。
- 任务、权限、审批、恢复、成功判定、凭据和持久化属于 Rust Host。
- 命令使用幂等键、revision CAS 和 Receipt；事件带 sequence/revision，支持重放和重启恢复。
- 当前仅交付桌面版；`WebHostAdapter` 只保留协议占位，不建设正式网页版或云端业务权威。

完整设计见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。

## 1.x 冻结范围

下列能力不是当前产品主线，保持隐藏或冻结：

- 创意中心；
- 无限画布；
- 图片、视频、音频媒体生成；
- 销帮帮、飞书及其他外部 CRM/协同系统集成；
- 正式网页版、D1 业务镜像、团队同步和跨设备协作。

现有共享底层代码在未完成依赖审计前不做破坏性删除，但不得继续扩展为用户入口。

## 开发与检查

环境要求：

- Windows x64；
- Node.js、pnpm；
- Rust stable 与 Tauri 2 所需 Windows 构建环境；
- 仓库内已准备并校验的官方 Codex sidecar。

常用命令：

```powershell
pnpm install --frozen-lockfile
pnpm desktop:dev
pnpm check
pnpm test
pnpm build
pnpm verify
pnpm desktop:build
pnpm release:build:internal
```

桌面构建目标为 NSIS。当前安装包在没有代码签名证书的情况下为**未签名安装包**，Windows 可能显示未知发布者提示；不得把未签名构建描述为已签名发布。

发布说明与安装包校验信息见 [docs/RELEASE_1.2.0.md](docs/RELEASE_1.2.0.md)。

## 数据与安全原则

- 不把 Provider、R2 或 Codex 凭据返回给 UI，也不写入业务 SQLite、LocalStorage、日志、诊断或 R2。
- 不向 UI 暴露原始 R2 URL、请求头或 Local Vault 绝对路径。
- 不可逆操作必须经过权限检查和必要审批。
- 客户端诊断只上报脱敏问题，不允许自行修改产品代码。
- SQLite 和 Local Vault 数据应随应用升级保留；发布、回滚和恢复操作不得以清空用户数据为前提。

## 许可证与第三方

第三方组件及归属见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 `licenses/`。产品借鉴成熟开源项目的交互结构与工程实践，但不复制 OpenAI 品牌、图标、专有文案或专有资产。
