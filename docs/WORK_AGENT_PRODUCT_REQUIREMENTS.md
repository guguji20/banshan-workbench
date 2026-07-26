# BSAIGC 工作 Agent 产品需求

## 1. 产品北极星

BSAIGC Desktop 不是一个表单集合，也不是在报价、合同、Brief、案例库旁边各放一个聊天框。

产品目标是提供一个类似 Codex 的本地工作 Agent：用户给出目标、文件或零散上下文后，Agent 能检查现有资料、制定步骤、选择工具、执行可恢复任务、引用证据、生成 Artifact、请求必要审批，并把结果写回统一项目上下文。

典型入口不是“先选择合同模块并填写 20 个字段”，而是：

```text
用户：审一下这个合同，重点看看付款、验收和版权风险。
用户：把这个标书拆一下，告诉我今天必须准备什么。
用户：把分秒针最近 12 版反馈归一下，找出客户真正没满意的点。
用户：给这个项目找 8 条公司做过的类似案例，再生成前端 Brief。
用户：把客户过稿脚本转成明天摄影能直接执行的一页单。
```

Agent 应当自行识别任务类型和项目上下文，复用 Task、Asset、Memory、Tool、Permission、Approval 和 Artifact 协议完成工作，而不是让用户重复搬运信息。

## 2. “像 Codex 一样聪明”的具体含义

这里的“聪明”不是回复像聊天机器人，而是完整的工作执行闭环：

1. **理解目标**：从自然语言、项目状态、文件和历史记录判断用户真正要完成什么。
2. **检查上下文**：先读取已有项目资料、历史案例、已确认 Brief、合同模板、版本反馈和任务状态，不让用户重复提供。
3. **发现缺口**：明确缺少的材料、冲突字段、模糊表达和高风险假设，只追问真正阻塞的问题。
4. **制定计划**：把任务拆成可追踪步骤，区分自动执行、人工复核和不可逆审批。
5. **调用工具**：选择文档解析、知识检索、规则审查、媒体分析、模板生成、任务创建等 Tool，而不是只生成一段文字。
6. **持续执行**：长任务进入 Durable Task Engine，支持进度、取消、恢复、重试和重启续跑。
7. **提供证据**：每个重要结论指向来源文件、页码、段落、时间点、版本或项目记录，并标注不确定性。
8. **生成成果**：输出结构化清单、风险报告、Brief、执行脚本、回复话术、DOCX/XLSX/PDF 或其他 Artifact。
9. **安全落地**：写文件、覆盖正式数据、改变关键业务状态、生成对外文件或发送给客户等操作必须经过权限和必要审批。
10. **形成记忆**：经确认的结论进入项目记忆或规则库；换模型不丢项目、Thread、历史和审计记录。

模块页面只是不同业务视图。主聊天、任务面板、Artifact 和项目记忆应当共用同一个 Agent Runtime，不能每个板块复制一套 AI。

## 3. 访谈问题对应的产品能力

### 3.1 资料反复找

需要统一的知识与素材能力，而不只是手工标签列表：

- 公司视频号和历史案例采集、去重、分类和语义检索；
- 按客户、行业、项目、内容类型、表现形式、演员、AIGC、质量等级检索；
- 脚本贴图、样片、平面、封面、定版、调色和音乐参考检索；
- 服装、道具库存、位置、状态和借还记录；
- Agent 根据当前需求主动组合查询并解释推荐理由；
- 检索结果必须返回稳定 `assetId`、来源、项目归属和可追溯依据；
- 后续可增加图像、音频、视频 embedding，但业务权威仍在本地 Vault 和 SQLite。

目标体验：

```text
给我找 6 条我们做过的高端地产人物采访片，最好有设计师、非 AIGC，节奏偏克制。
```

Agent 返回候选、匹配理由、可复用镜头/结构、差异和原始资产入口，而不是让编导重新翻收藏夹。

### 3.2 需求反复确认

需要 Agent 驱动的需求澄清和交接，而不只是固定表单：

- 读取会议纪要、聊天记录、语音转写、市场备注和历史项目；
- 自动整理结构化 Requirement Brief；
- 识别目标、受众、交付物、时间、预算、风格、禁区、验收标准等缺口；
- 对“高级、不好看、调性不对”等模糊反馈生成有区分度的追问；
- 记录需求版本、变化来源、确认人和影响范围；
- 市场到编导、编导到摄影/后期/平面的交接必须有理解确认；
- 非专业常见问题优先由市场话术库和 Agent 处理，减少对创作者的即时打断。

目标体验：

```text
把今天客户会里的内容整理成 Brief，只问我还缺的关键问题，并告诉我哪些变化会导致重拍或超预算。
```

### 3.3 信息反复录入

商务 Workspace 是唯一业务主数据源，Agent 负责消灭重复录入和状态搬运：

- 从合同、标书、报价、聊天和项目 Brief 中提取客户、金额、时间、交付和付款字段；
- 对同客户历史资料提供可解释预填；
- 生成报价、合同、请款、验收和回款记录；
- 已确认字段在后续流程中自动复用，不要求跨页面重复填写；
- 客户、项目、金额、状态和文档变更必须保留来源、版本和审计记录；
- 提供标准导入、导出和备份能力，但不依赖销帮帮、飞书或外部回款表；
- 所有文档生成前执行一致性检查，避免名称、金额、税率、日期和项目编号不一致。

目标体验：

```text
这个项目已经签了。把合同里的金额、付款节点和验收条款录进项目，生成首款请款资料，并更新项目状态和后续回款待办。
```

### 3.4 创作频繁被打断

需要事件驱动的工作收件箱和异步汇总：

- 将客户反馈、内部问题、版本结果和临时事项收进项目 Inbox；
- Agent 合并重复问题，按阻塞程度和负责人分类；
- 创作者专注期间不持续弹出低优先级消息；
- 在用户主动查看或约定事件到达时生成摘要；
- 简单客户问题可生成市场部回复草稿；
- 需要编导、摄影、后期判断的问题形成明确任务和截止时间；
- 大脑按事件唤醒，不持续监控、不持续耗费模型和流量。

目标体验：

```text
我刚写完脚本。把这两小时新增反馈整理一下，只告诉我现在会阻塞明天拍摄的事情。
```

## 4. 高价值工作流清单

### 4.1 市场部 / 客户经理

1. **合同审查**：提取关键条款、检查风险和缺失、与公司规则/模板比对、生成修改建议和沟通清单。
2. **标书拆解**：提取截止时间、资格条件、必交材料、报价表、授权、盖章、装订和提交方式，形成“AI 初筛 + 人工确认” checklist。
3. **报价生成**：根据需求和历史报价推荐模板、拆项和异常项，生成草稿并检查金额计算。
4. **请款/验收生成**：复用项目主数据和付款记录，生成文件并检查附件缺口。
5. **回款跟进**：根据付款节点生成待办和提醒，不自动对外发送。
6. **内部经营台账**：统一查询客户、项目、合同额、应收、已收、风险、待办和归档状态。
7. **案例推荐**：按客户需求搜索公司历史案例并生成推荐理由。
8. **成片发出前审核**：logo、周年标识、人物、车辆、项目名称、字幕、音乐和授权检查。

### 4.2 编导

1. **需求会议整理与补问**；
2. **样片、脚本贴图和视觉参考检索**；
3. **脚本/方案辅助生成与事实检查**；
4. **客户模糊反馈拆解和追问建议**；
5. **分秒针反馈汇总、去重和版本复盘**；
6. **把客户意见转译为后期、平面和动画可执行任务**；
7. **接收方理解确认与差异检查**；
8. **客户常见问题回复草稿**；
9. **服装、道具查询和借还任务**；
10. **客户过稿脚本转现场执行脚本**。

### 4.3 摄影 / 现场执行

1. 拍前 24 小时材料完整性检查；
2. 从客户脚本生成一页执行脚本；
3. 标记主镜头、次镜头、必拍、保底和可替代镜头；
4. 根据场地、演员、时间和器材识别风险；
5. 生成等待时间利用清单；
6. 拍后标记重点素材、设计镜头和使用建议；
7. 将素材亮点传给后期并保留编导/摄影意图。

## 5. 合同审查 Agent：首个智能工作流

合同审查应作为下一条首要纵向闭环。它同时验证文档理解、规则检索、证据引用、Agent Tool 调度、Artifact、审批和项目记忆，是判断系统是否真的“像 Codex 会干活”的合适试金石。

### 5.1 用户输入

- DOCX、PDF、扫描 PDF 或图片；
- 可选的项目、客户和商务 Workspace；
- 用户自然语言关注点，例如付款、验收、版权、违约、保密；
- 公司合同模板、条款规则、风险偏好和历史已确认案例；
- 可选的对方修订版或上一版本。

### 5.2 Agent 执行步骤

1. 将原件导入 Vault，记录 hash、来源和项目归属；
2. 识别文件类型，提取段落、表格、标题、页码和批注；
3. 扫描件进入 OCR，并保存文字到原始页面的定位关系；
4. 判断合同类型、当事方、金额、税率、期限、交付、付款、验收和附件；
5. 检查同一合同内部的名称、数字、日期、编号和引用是否一致；
6. 从经批准的公司规则包和模板中检索相关规则；
7. 逐条生成风险发现，附来源位置、规则依据、严重级别、置信度和影响；
8. 识别缺失条款、空白字段、附件缺口和需要业务人员确认的问题；
9. 生成建议修改文本、对客户沟通话术和内部处理任务；
10. 生成结构化审查报告 Artifact；
11. 涉及改写原文件、生成对外版本或写入业务数据时请求审批；
12. 经确认的决定写入项目记忆和规则反馈，不把模型原始猜测直接当作权威。

### 5.3 必查范围

- 主体名称、签约资格和联系人；
- 项目名称、服务内容、数量和交付物；
- 合同金额、税率、含税/不含税和大小写金额；
- 付款比例、付款条件、开票条件和账期；
- 项目周期、交付日期和延误责任；
- 验收标准、验收期限、默认验收和修改次数；
- 知识产权、素材授权、肖像权、音乐和第三方资产；
- 保密、数据和宣传案例使用；
- 违约责任、赔偿上限和责任是否对等；
- 取消、终止、不可抗力和已发生成本；
- 争议解决、适用法律和管辖；
- 附件、报价单、需求单和正文引用的一致性；
- 空白、占位符、未定义术语和前后矛盾。

### 5.4 每条发现的数据结构

```text
findingId
severity            blocker | high | medium | low | info
category
summary
impact
sourceAssetId
sourceLocator        page / section / paragraph / table cell
sourceExcerpt
rulePackId
ruleId
ruleVersion
confidence
suggestedAction
suggestedWording
requiresHumanDecision
status               open | accepted | dismissed | resolved
reviewedBy
reviewedAt
```

重要结论不得只有模型总结，必须能定位到原合同和对应规则。无法确认时必须标记不确定，而不是补造条款。

### 5.5 输出成果

- 一页合同概览；
- 风险分级清单；
- 缺失资料与待确认问题；
- 金额、付款、日期和交付结构化表；
- 给客户的沟通话术草稿；
- 内部任务和负责人建议；
- 修订条款建议；
- 可选 DOCX/PDF 审查报告；
- 经审批后生成的修订稿或对比稿。

### 5.6 安全边界

- Agent 不替代法务或业务负责人作最终法律判断；
- 不自动签署或发送对外文件；
- 原始合同和提取文本默认留在本地 Vault；
- 规则包必须有版本和来源；
- 模型切换不得改变已确认 Finding、审批和 Artifact；
- 重新运行必须可追踪规则版本、模型和输入资产；
- 敏感信息不得进入诊断上报或普通日志。

## 6. 标书拆解 Agent

标书工作流与合同审查共享文档解析、证据定位和规则引擎，但输出不同：

- 招标人、项目、预算、包件和截止时间；
- 报名、答疑、保证金、递交和开标时间；
- 资格、业绩、人员、设备和财务要求；
- 必交证照、授权书、承诺函和声明；
- 报价表、分项表和计算关系；
- 签字、盖章、密封、装订、份数和电子介质要求；
- 废标条件和高风险遗漏；
- 原文页码与条款证据；
- AI 初筛、人工确认状态和负责人；
- 按截止时间倒排的任务计划。

系统必须明确“AI 初筛，不替代人工复核”，并让每一项 checklist 可回到采购文件原文。

## 7. 分秒针反馈与版本复盘 Agent

核心不是把评论复制成列表，而是恢复多版本因果关系：

1. 导入或粘贴每个版本的反馈；
2. 识别反馈人、时间、视频版本和时间码；
3. 合并同义、重复和已解决意见；
4. 标注新增、反复出现、被推翻和互相冲突的意见；
5. 区分事实错误、品牌规范、审美偏好和方向变化；
6. 将“高级、不好看、节奏不对”等模糊意见转成追问；
7. 生成给后期的可执行修改项；
8. 识别真正卡点和版本膨胀原因；
9. 保存客户决策和历史版本，避免旧意见再次回流。

输出至少包括：本轮必须改、需要确认、已解决、冲突意见、方向变化、潜在重做风险和客户回复草稿。

## 8. 统一 Agent 运行架构

所有上述工作流应走同一条链：

```text
用户目标 / 文件 / 事件
→ System Brain Thread
→ Intent + Context Resolver
→ Agent Plan
→ Tool Registry / Skill Registry
→ Durable Task DAG
→ Asset / Document / Knowledge / Business Tools
→ Evidence + Artifact
→ Approval（必要时）
→ 项目状态与 Memory 更新
```

React 只展示：

- 对话；
- Agent 计划；
- 任务进度；
- 待确认问题；
- 风险与证据；
- Approval；
- Artifact；
- 项目视图。

React 不实现合同规则、客户匹配、差异计算、任务恢复、文档解析或 Tool 调度。

## 9. 需要新增的通用能力

以下名称是下一阶段设计候选，冻结协议前必须落到 commands、events、permissions、tools 和 storage：

### 9.1 Document Intelligence

- 文档导入、格式识别和安全检查；
- DOCX/PDF/XLSX/图片文本与结构提取；
- OCR；
- 页码、段落、表格和批注定位；
- 文档版本比较；
- 结构化字段提取；
- 大文档分块和本地缓存。

候选 Tool：

```text
document.extract
document.ocr
document.compare
document.locate
document.extractFields
```

### 9.2 Knowledge and Retrieval

- 项目、案例、合同、规则、脚本和参考资产索引；
- 元数据过滤与全文/语义混合检索；
- 权限过滤；
- 来源引用；
- 索引增量更新和重建；
- 模型/embedding 更换后的可迁移索引策略。

候选 Tool：

```text
knowledge.search
knowledge.getSource
case.search
reference.search
rule.search
```

### 9.3 Review Engine

- 版本化 Rule Pack；
- 确定性检查与模型判断分离；
- Finding 生命周期；
- 人工确认、驳回和豁免理由；
- 可重复运行和差异比较；
- 审查报告 Artifact。

候选 Tool：

```text
contract.review
contract.suggestRevision
tender.extractChecklist
feedback.consolidate
feedback.compareVersions
quality.preflight
```

### 9.4 Agent Tool Dispatch

- 从 Module Registry 发现可用 Tool；
- 将 Codex app-server tool call 映射到 Backend Host；
- 参数 schema 验证；
- 项目、窗口、操作和资源权限检查；
- deadline、取消和 Task 绑定；
- 结果脱敏和轻量化；
- 不可逆 Tool 的 Approval；
- Tool Call、结果和 Artifact 审计；
- 崩溃或重启后的恢复语义。

### 9.5 Business Ledger 与 Archive

- 客户和项目主数据；
- 报价、合同、请款、验收和回款状态；
- 应收、已收、逾期、风险和待办统计；
- 项目归档、历史版本和审计；
- 标准 XLSX/CSV/PDF 导出和本地备份；
- 历史项目批量导入与字段校验。

商务系统自身是唯一业务权威，不依赖销帮帮、飞书、外部回款表或第三方连接器。

## 10. 当前实现与真实缺口

| 能力 | 当前状态 | 说明 |
|---|---|---|
| Rust Backend Host / typed IPC | 已有 | 通用地基可复用 |
| SQLite Ledger / Vault | 已有 | 本地权威已建立 |
| Durable Task / cancel / retry / recovery | 已有 | 可承载长文档任务 |
| Official Codex app-server Thread/Turn | 已有基础 | 会话可运行，产品凭据和正式 sidecar 尚未完成 |
| Approval Ledger | 已有 | 可复用到正式文档、关键状态修改和合同修订 |
| Requirement Brief | Headless 已有 | 缺 Agent 自动整理、缺口识别和最终 UI |
| Case Library | Headless 已有 | 缺自动采集、内容理解和语义检索 |
| Execution Brief | Headless 已有 | 缺从脚本/场地资料自动生成和拍前检查 |
| Business Document Center | Headless 已有 | 可生成内置模板，缺真实模板、智能审查和完整业务 UI |
| Agent dynamic tool dispatch | 未实现 | 当前 Module tools 主要是声明性名录 |
| Document Intelligence / OCR | 未实现 | 合同、标书审查的直接阻塞项 |
| Contract Review | 未实现 | 只有合同生成和状态，不等于审合同 |
| Tender Review | 未实现 | 没有采购文件拆解和证据 checklist |
| Feedback Version Review | 未实现 | 没有分秒针多版本汇总与因果复盘 |
| Knowledge semantic retrieval | 未实现 | 现有案例库主要是结构化元数据 |
| Internal Business Ledger | 部分已有 | 缺经营看板、归档、导入导出和完整查询 |
| 最终业务 UI | 未实现 | 当前不冻结视觉和交互 |
| Web / team collaboration | 未实现 | 只保留 WebHostAdapter 后路 |

## 11. 下一阶段开发顺序

### 阶段 A：Agent 能真正调用产品工具

1. 冻结 Tool Call、Tool Result、Evidence、Artifact 和 Agent Task 绑定协议；
2. 实现 Codex app-server → Tool Registry → Backend Host 动态调度；
3. 接入权限、Approval、deadline、取消、恢复和审计；
4. 建立 Skill 包加载和项目上下文装配；
5. 用只读 `project.get`、`asset.get`、`businessWorkspace.list` 做安全探针。

### 阶段 B：合同审查纵向闭环

1. Document Intelligence：DOCX/PDF 提取和定位；
2. Rule Pack 与 Finding 协议；
3. 合同字段和确定性一致性检查；
4. Agent 风险审查和证据引用；
5. 审查报告 Artifact；
6. 建议修改与 Approval；
7. 真实合同匿名样本回归测试。

### 阶段 C：复制能力到标书和反馈

1. 标书 checklist；
2. 分秒针反馈汇总和版本复盘；
3. 成片发出前质量检查；
4. 将 Findings、Evidence 和 Artifact 复用到三条工作流。

### 阶段 D：资料检索和创作交接

1. 案例/样片/脚本贴图语义检索；
2. Requirement Brief Agent；
3. 客户反馈追问和回复话术；
4. 脚本到 Execution Brief；
5. 拍后素材亮点传递。

### 阶段 E：商务经营台账与稳定化

1. 客户、项目、合同额、应收、已收和风险经营看板；
2. 全流程查询、筛选、待办和归档；
3. 标准导入、导出和本地备份；
4. 真实业务样本回归和误报漏报修复；
5. 桌面安装、升级、恢复和发布门禁。

## 12. 首阶段验收场景

系统只有通过以下场景，才算从“有 Codex 会话”升级为“像 Codex 的工作 Agent”：

1. 用户拖入合同并说“审一下”，不需要先手工选择十几个功能；
2. Agent 自动读取项目、客户和商务 Workspace 上下文；
3. Agent 展示可追踪计划并创建 Durable Task；
4. 重启应用后任务、Thread、进度和已生成 Finding 能恢复；
5. 每个高风险结论能跳回合同页码/段落和规则依据；
6. 缺材料时只问阻塞问题，不要求重新录入已有信息；
7. 可以生成审查报告、客户回复草稿和内部任务；
8. 修改原文、生成对外版本或改变关键业务状态前必须审批；
9. 模型切换后项目、Findings、Artifact 和审批不丢；
10. Provider、Codex sidecar、断网或应用重启不破坏本地原件和已完成成果。

这份文档是重复工作和智能 Agent 产品主线。单个业务模块的实现不得把产品退化成孤立表单，也不得绕过统一 Task、Tool、Memory、Permission、Approval、Asset 和 Artifact 系统。

## 13. 商务阶段开源复用决定

商务闭环不得把 Document Intelligence、OCR、Office 处理、Agent 消息 UI、动态表单、数据表格、规则引擎、搜索和诊断全部从零实现。完整候选、许可证、集成方式、淘汰条件和 PoC 见 [BUSINESS_AGENT_OPEN_SOURCE_REUSE.md](./BUSINESS_AGENT_OPEN_SOURCE_REUSE.md)。

冻结原则：

1. 官方 Codex app-server 继续是唯一 Agent Runtime；
2. 本地 SQLite Task Ledger、Vault 和 Business Workspace 继续是唯一权威；
3. 已核验 129 个 GitHub 候选；优先 Direct Dependency，其次纯 JSON Isolated Sidecar；
4. MIT/Apache 代码可选择性复用，但必须保留归属和修改记录；
5. GPL/AGPL/Elastic/商业混合项目仅做 clean-room 业务设计参考；
6. 第三方 parser、OCR、Office 和 UI 数据结构必须经 BSAIGC adapter 归一化；
7. 大规模业务开发前先完成中文合同解析、扫描 OCR、真实 Office 模板、Agent UI 四个 PoC；RapidOCR、Cedar、promptfoo、restic/age 分别进入轻量 OCR、授权、评测和备份对照；
8. React Admin/Atomic CRM 只有在不形成第二状态权威、不锁死视觉且只通过 Client SDK 访问后端时才采用；
9. Paperclip 只借 Attention Inbox、Approval、Task/Activity 工作台，不接管 Runtime；
10. 对外正式安装包必须生成 THIRD_PARTY_NOTICES、SBOM、模型清单和 sidecar manifest。

商务 1.0 推荐组合：

```text
Codex app-server
+ assistant-ui / TanStack Table / RJSF
+ Docling / PaddleOCR / RapidOCR / LiteParse / PDF.js
+ Open XML SDK / PowerTools / ClosedXML / MiniWord
+ Calamine / rust_xlsxwriter / Typst
+ GoRules Zen / Cedar PoC
+ SQLite FTS5 / Tantivy / sqlite-vec / USearch 条件 PoC
+ promptfoo 回归评测 / restic + age 备份 PoC
+ 现有 Rust Host / Task / Vault / Ledger
```

开源复用节省通用底层开发，但不替代公司合同规则、报价审批规则、真实模板映射、业务状态机、Evidence/Artifact adapter 和真实样本回归。
