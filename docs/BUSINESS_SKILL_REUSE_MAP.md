# 商务 Skill 开源复用映射

> 更新日期：2026-07-22

## 结论

- Anybox（MIT）可直接复用的是 Skill frontmatter、按需加载、资源路径安全、Connector 最小权限和 Registry 结构；它内置的业务 Skill 很少，主要是 Feishu/Gmail Connector 指南。
- Proma（AGPL-3.0，部分文档 Skill 另有专有许可）只作为 clean-room 产品参考。未复制其源码、Prompt、脚本或模板。
- BSAIGC 继续使用官方 Codex app-server 作为唯一 Agent Runtime，不引入 Anybox/Proma Runtime、自动化引擎或子 Agent 系统。

## 已落地的商务化映射

| 参考能力 | BSAIGC 商务 Skill | 处理方式 |
|---|---|---|
| Anybox Skill frontmatter / Registry | 全部内置商务 Skill | MIT 结构思想，BSAIGC 原创内容 |
| Anybox Feishu/Gmail 最小权限 Connector | `business-communication-drafter` | 当前只消费已授权输入，不接飞书/Gmail运行时 |
| Proma PDF/DOCX/XLSX 文档工作流 | `business-document-pipeline` | clean-room 重写为识别、提取、生成、渲染和验证闭环 |
| Proma brainstorming / writing-plans / executing-plans | `business-case-orchestrator` | clean-room 合并为阶段编排、检查点和恢复；复用现有 Task/Artifact |
| Proma collaboration 的独立核验思路 | `business-review-board`、`business-consistency-audit` | 复用官方 Codex Thread/Task/Approval，不引入第二套 Runtime |
| Proma automation 的周期跟进思路 | `receivables-followup` | 当前只生成跟进计划和草稿，不做无人值守外发 |

## 当前 Bundle

`src-tauri/resources/business-skills/bundle.json` 版本 `1.2.0`，共 14 个默认商务 Skill。

新增：

1. `business-document-pipeline`
2. `business-case-orchestrator`
3. `business-communication-drafter`
4. `contract-version-compare`
5. `receivables-followup`
6. `business-review-board`

原有：

1. `business-brief-intake`
2. `quotation-builder`
3. `contract-review`
4. `project-master-data`
5. `tender-checklist`
6. `payment-request-pack`
7. `acceptance-pack`
8. `business-consistency-audit`

## 边界

- Skills 只声明业务流程、工具、权限、审批边界和 Artifact。
- 所有真实执行仍通过 BSAIGC Client SDK、Rust Host、统一 Tool Registry、Task Ledger 和 Vault。
- 外发、最终金额、合同签署、到账认领、验收通过等不可逆动作必须人工确认。

## 已筛选的内置 Skill

### Anybox（MIT，可选择性复用结构）

已转成商务工作流的来源能力：

| Anybox 内置能力 | 商务化去向 | 处理结果 |
|---|---|---|
| `pdf` | `business-document-pipeline`、`contract-review` | 只保留文档接收、页级证据和失败标记 |
| `transcribe` | `business-brief-intake` 的后续输入适配 | 当前版本只预留媒体转写接口，不把转写运行时塞进工作台 |
| `feishu` / `gmail` Connector | `business-communication-drafter`、飞书渠道壳 | 只接收已授权输入，当前不主动外发 |
| Skill frontmatter / plugin manifest / Connector 最小权限 | 全部商务 Skill | 已重写为 BSAIGC manifest 和统一 Tool Registry |
| automation / scheduler | `receivables-followup`、`business-case-orchestrator` | 改为 Task Ledger 计划，禁止第二套后台调度器 |

未采用的 Anybox 技能（移动端、SwiftUI、知乎发布、腾讯部署等）与商务闭环无关，保持不安装、不加载。

### Proma（AGPL；部分文档 Skill 另有专有许可）

只做 clean-room 参考，不复制源码、Prompt、脚本或模板：

| Proma 内置能力 | 商务化去向 |
|---|---|
| `pdf` / `docx` / `xlsx` | `business-document-pipeline`、`quotation-builder`、`payment-request-pack`、`acceptance-pack` |
| `brainstorming` / `writing-plans` / `executing-plans` | `business-case-orchestrator` |
| `agent-collaboration` | `business-review-board`、`business-consistency-audit` |
| `automation` | `receivables-followup`，只生成可审核计划 |
| `find-skills` / `skill-creator` / `tool-builder` | 开发期能力，不进入商务用户 Skill Bundle |

因此当前 Bundle 不再复制“通用助手技能”，只发布商务用户真正需要的 14 个 Skill；后续新增 Skill 必须同时具备真实 Tool、权限和 Artifact 产出，不能只加一份 Markdown。
