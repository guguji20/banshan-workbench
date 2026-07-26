# 半山商务工作台 1.2.0 发布说明

- 产品：半山商务工作台
- 版本：`1.2.0`
- 发布日期：`2026-07-22`
- 当前状态：**正式内测发布，最终安装包验收通过**
- releaseStatus: final-internal-accepted

## 本次交付

### 商务闭环

- 客户主数据、需求同步、报价确认、合同、请款、验收、交付、发票、到账、回款、台账和归档统一进入本地 Business Workspace。
- 客户身份以稳定客户 ID、法定名称和税务身份维护；历史工作区可作为受控预填来源，候选、差异预览和采用决定均可追踪。
- 交付物按版本登记，发送批次和客户签收凭证独立留痕；里程碑、验收与交付状态参与归档前置检查。
- 发票支持开具、红冲和附件绑定；到账采用不可变流水账，支持分次到账、冲销、重复流水号拦截和客户应收汇总。
- 业务状态迁移继续执行幂等键、revision CAS、Receipt、事件顺序和重启恢复约束。

### 归档与资产完整性

- 归档前必须完成生效合同、验收、全额回款、待处理单据清理和完整性预检。
- 归档快照生成 ZIP、外部 Manifest、包内 Manifest、业务快照和逐项 SHA-256；归档后工作区转为只读。
- 导出或正式归档前重新校验 Local Vault、包大小、Manifest 字段、ZIP 条目和摘要，篡改、缺失或重复关联会被拒绝。
- 用户导入、业务生成物、审查报告、归档包和归档 Manifest 使用显式来源与类型记录；重启对账只清理未提交或失去关联的生成物，不依赖文件名前缀猜测来源。
- Local Vault 与 SQLite 仍是唯一业务权威；R2 仅通过 Durable Backup Outbox 异步备份和显式恢复，失败不阻断本地闭环。

### Agent、技能与合同审查

- 官方 Codex app-server `0.144.5` 仍是唯一 Agent Runtime，业务动态工具只发布显式 allowlist。
- 内置 14 个生产 Business Skills 和 14 个业务工具，覆盖案件编排、客户主数据、需求访谈、报价、合同审查、版本比较、请款、验收、应收跟进、文档流水线和一致性审计。
- 合同审查保留确定性规则、结构化 Evidence、人工逐条决策、HTML/DOCX 报告和无凭据降级闭环；Agent 失败不会覆盖规则结果或阻断本地报告生成。
- 工具输入、输出和错误统一拦截本地绝对路径、URL、凭据和越权资源绑定；写操作按工具声明执行权限和审批检查。

### AI、设置与本地安全

- 设置中心支持多 OpenAI-compatible Provider、模型列表、默认模型切换和连接状态展示。
- 内测 Provider 首次启动后只写入当前 Windows 用户的 DPAPI 保护区；验收已在全新隔离 Profile 中完成解密、schema、Provider、HTTPS endpoint 和默认模型一致性校验。
- SQLite、Local Vault、缓存、暂存区、凭据区和更新状态集中展示；只允许通过白名单目标打开目录或清理可再生缓存。
- UI 仅接收稳定 ID、状态和轻量 JSON，不返回 Provider、Codex、R2 凭据或 Local Vault 绝对路径。

## 安装包

- 文件名：`半山商务工作台_1.2.0_x64-setup.exe`
- 文件大小：`131240754` bytes
- SHA-256：`cc06d501ac5a1e67dbfb103c2701d037b487c74c45bd96b1bc8d8dd26338b0e5`
- Authenticode：**NotSigned（未签名）**

产品安装包尚未配置公司代码签名证书，Windows 可能显示未知发布者或 SmartScreen 提示。官方 Codex sidecar 自身的 Authenticode 状态为 `Valid`，签名主体为 OpenAI OpCo, LLC。

## 质量门

- 协议生成：`253 passed`
- 前端：`21 files / 124 tests passed`
- Rust：`574 passed / 0 failed / 4 ignored`
- Business Tool Registry：`24 passed`
- Business Skills：`14 skills / 14 tools / 35 files` 校验通过
- TypeScript、Vite build、Rust fmt/check/clippy：全部通过
- 官方 Codex app-server 真实握手：`1 passed`
- Codex sidecar Manifest、SHA-256、LICENSE、NOTICE、Authenticode：全部通过
- `git diff --check`：通过

Vite 当前仍报告主 JavaScript chunk 约 `527.40 kB` 的体积提示；该提示不影响本次功能与安装包验收，后续可按页面边界做按需加载优化。

## 跨版本验收

- RunId：`release-1.2.0-20260723-final`
- 升级类型：`cross-version-upgrade`
- 初始版本：`1.1.0`
- 最终版本：`1.2.0`
- 旧包 SHA-256：`759eb2b69bf736d4199e08c0c9ca6aec547c3a12a1207b2dc806cfeacab3dfda`
- 最终包 SHA-256：`cc06d501ac5a1e67dbfb103c2701d037b487c74c45bd96b1bc8d8dd26338b0e5`
- 结果：`passed`

验收覆盖旧包隔离安装与首启、SQLite/Vault 建立、最终包覆盖升级、升级前后权威数据快照一致、内测 Provider DPAPI 状态验证、Codex LICENSE/NOTICE 校验、两次独立 PID 重启、静默卸载、测试注册表清理和既有注册表快照恢复。

## 内测边界

- 本安装包包含内测 API 凭据，只允许公司内部受控分发；不得公开上传或对外发送。
- R2、飞书 CLI 和在线更新仍以本地优先与显式状态为边界，不得把未配置通道显示为已连接。
- 网页版未实现；`WebHostAdapter` 仅用于冻结协议和测试可迁移边界。
- 正式外发前必须配置产品代码签名、轮换内测凭据，并接入可撤销的凭据分发机制。
