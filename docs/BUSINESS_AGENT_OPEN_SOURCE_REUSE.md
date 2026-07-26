# 商务 Agent 开源复用与技术选型

> 状态：候选冻结前审计稿  
> 核验日期：2026-07-19  
> 适用范围：BSAIGC Desktop 商务全流程闭环  
> 原则：优先成熟依赖和隔离 sidecar，最少自研；GPL/AGPL/商业混合项目只做 clean-room 设计参考。

## 1. 决策结论

商务系统不再按“所有能力从零写”推进，而按以下优先级实施：

```text
官方/成熟 Package 直接依赖
→ 自包含 Sidecar
→ MIT/Apache 小范围选择性复用
→ GPL/AGPL/商业项目 clean-room 设计参考
→ 只有确实没有成熟实现的 BSAIGC 业务胶水才自研
```

截至 2026-07-19，已通过 GitHub 官方仓库/API 核验 **129 个互不重复的候选仓库**：首轮 93 个，第二轮再扩展 36 个。元数据和许可证快照保存在：

```text
.runtime/open_source_github_metadata.json
.runtime/open_source_github_metadata_expansion.json
.runtime/open_source_github_metadata_expansion2.json
.runtime/open_source_github_metadata_expansion3.json
.runtime/open_source_expansion3_license_audit.json
.runtime/open_source_license_api.json
.runtime/open_source_license_text_summary.json
.runtime/open_source_expansion_license_text.json
.runtime/licenses/
```

### 1.1 冻结的主组合

```text
Agent Runtime
  官方 Codex app-server（现有边界，不再自研第二套 Runtime）

商务 Agent UI
  assistant-ui + TanStack Table + RJSF
  可选：React Admin/Atomic CRM PoC
  参考：Paperclip Attention Inbox / Approval / Task 工作台

文档理解
  Docling（复杂合同结构、表格和阅读顺序主引擎）
  + PaddleOCR（中文扫描件 OCR 主引擎）
  + RapidOCR（Windows/ONNX 轻量 OCR 对照组）
  + LiteParse（Rust-native 快速路径对照组）
  + Microsoft Presidio（脱敏）
  + PDF.js / react-pdf-highlighter（预览和证据定位）
  Kreuzberg / OpenDataLoader fast 仅在真实样本 PoC 胜出时替换对应路径

Office 与 Artifact
  .NET self-contained Office sidecar：
    Open XML SDK + Open-Xml-PowerTools + ClosedXML + MiniWord
  Rust：Calamine + rust_xlsxwriter
  PDF/报告：Typst
  格式转换：LibreOffice headless（隔离、人工许可证审批）
  PDF 基础处理：qpdf；pdfcpu 为备选

业务规则与数据
  ERPNext / Dolibarr / Twenty / Axelor / Invoice Ninja 只借领域模型
  GoRules Zen 做确定性规则 PoC
  SQLx + SQLite 继续做权威数据源
  SQLite FTS5 / Tantivy 做全文检索
  sqlite-vec 做本地语义检索 PoC；LanceDB 为备选

诊断与经营展示
  OpenTelemetry Rust
  Apache ECharts
```

### 1.2 不引入的第二套平台

以下项目再成熟，也不允许成为另一套业务或 Agent 事实源：

- ERPNext、Odoo、Dolibarr、Twenty：不整体嵌入，不复制业务代码；
- Paperclip、OpenSwarm：不接管 Codex Runtime、Task、Approval 或数据库；
- Temporal、Restate、Meilisearch、Qdrant：桌面 1.0 不引入独立服务集群；
- Documenso、DocuSeal、OpenSign：电子签署不进入商务 1.0 核心；
- CopilotKit、Vercel AI SDK、BAML：不引入第二套 Agent Runtime；
- Paperless-ngx、Docspell、Mayan EDMS：不整体部署为文档权威；
- Node-RED、Trigger.dev、Kestra：不接管本地 Task Ledger、Tool Registry 或 Approval；
- OPA、SpiceDB：桌面 1.0 不启动额外权限服务；Cedar 仅作为 Rust-native policy PoC；
- DuckDB：1.0 不增加第二业务数据库，仅在后续只读分析量达到阈值时再评估。

## 2. 复用级别

| 级别 | 含义 | 默认做法 |
|---|---|---|
| Direct Dependency | 直接成为 Cargo/NPM/NuGet/Python 依赖 | 固定版本和 lockfile，执行 SBOM/许可证扫描 |
| Isolated Sidecar | 独立进程，通过纯 JSON/文件 Artifact 通信 | Host 管理启动、超时、取消、恢复、版本和校验和 |
| Selective MIT Reuse | 只复用少量 MIT/Apache 源码或组件 | 保留版权、LICENSE、NOTICE 和修改记录，禁止无边界复制 |
| Design Reference Only | 只研究流程、字段语义、状态机、交互和测试场景 | clean-room 重新实现，不复制代码、migration、样式或模板 |
| Reject | 当前产品不采用 | 许可证、架构、维护或嵌入成本不合适 |

## 3. 商务领域模型

这些成熟系统的价值是减少业务建模错误，不是直接搬入 PHP/Python/Java 大型应用。

| 能力 | 主选参考 | 备选参考 | License 风险 | 集成方式 | 复用级别 | 采用内容 |
|---|---|---|---|---|---|---|
| 商务全流程 | [ERPNext](https://github.com/frappe/erpnext) | [Odoo](https://github.com/odoo/odoo) | GPL-3.0 / LGPL-3.0 | 不集成 | Design Reference Only | Opportunity→Quotation→Order/Contract→Invoice→Payment→GL 的对象关系、状态和测试场景 |
| 轻量单据闭环 | [Dolibarr](https://github.com/Dolibarr/dolibarr) | [SolidInvoice](https://github.com/SolidInvoice/SolidInvoice) | GPL-3.0 / MIT | 不整体集成 | Design Reference Only | 中小企业单据、状态、列表和操作口径 |
| CRM 记录体验 | [Twenty](https://github.com/twentyhq/twenty) | [Frappe CRM](https://github.com/frappe/crm) | AGPL/Enterprise 混合 / AGPL | 不集成 | Design Reference Only | Company、Contact、Opportunity、Pipeline、Timeline、字段扩展 UX |
| 合同与周期计费 | [Axelor Open Suite](https://github.com/axelor/axelor-open-suite) | ERPNext | AGPL-3.0 / GPL-3.0 | 不集成 | Design Reference Only | 合同版本、终止、周期、批次开票和变更语义 |
| 报价/发票/付款 | [Invoice Ninja](https://github.com/invoiceninja/invoiceninja) | [InvoicePlane](https://github.com/InvoicePlane/InvoicePlane) | Elastic License / 自定义许可 | 不集成 | Design Reference Only | 报价、PDF、付款、客户门户、回调与失败场景 |
| CRM 权限/实体元数据 | [EspoCRM](https://github.com/espocrm/espocrm) | [SuiteCRM](https://github.com/salesagility/SuiteCRM-Core) | AGPL-3.0 | 不集成 | Design Reference Only | 角色、记录级 ACL、关系和可扩展实体 |
| 订阅/账单事件 | [Kill Bill](https://github.com/killbill/killbill) | Invoice Ninja | Apache-2.0 / Elastic | 仅研究，不启动 Java 服务 | Design Reference Only | 账单事件、幂等、退款、失败重试和审计测试 |
| 双重记账概念 | ERPNext / Odoo | [Medici](https://github.com/flash-oss/medici) | GPL/LGPL / MIT | 不作为 1.0 账本实现 | Design Reference Only | journal、分录、撤销、核销；经营台账不得冒充会计总账 |
| 引导式需求采集与文档装配 | [docassemble](https://github.com/jhpyle/docassemble) | RJSF/自有 Agent 追问 | MIT | 不整体部署其 Web/Python 平台 | Design Reference Only | YAML interview、问题依赖、缺口追问、答案复用、文档装配和可解释访谈路径 |
| 企业流程与采购语义 | [Apache OFBiz](https://github.com/apache/ofbiz-framework) | ERPNext | Apache-2.0 | 不启动 Java ERP | Design Reference Only | Party、Order、Agreement、Invoice、Payment、Shipment/Deliverable、采购与审批语义 |
| 应收与计费边界 | [Bigcapital](https://github.com/bigcapitalhq/bigcapital) / [Lago](https://github.com/getlago/lago) | Kill Bill | AGPL-3.0 | 不集成 | Design Reference Only | 应收账龄、账单事件、退款、credit、收入看板和失败状态；只借模型与测试场景 |

### 3.1 必须照成熟系统保留的规则

1. 正式报价、合同、请款、验收提交后冻结，不原地覆盖；
2. 修改使用新版本、amendment、cancel、supersede、credit/reversal；
3. 每次转换记录 `sourceDocumentId`、`sourceRevision`、`idempotencyKey`；
4. 收款和应收支持多对多、部分到账、预付款、未分配余额、退款和撤销；
5. 所有金额使用精确十进制，不使用 float；
6. “经营台账”与“会计总账”在命名、数据结构和承诺上分开。

## 4. 商务工作台和 Agent UI

| 能力 | 主选 | 备选 | License | 集成方式 | 复用级别 | 决策 |
|---|---|---|---|---|---|---|
| Agent 主聊天 | [assistant-ui](https://github.com/assistant-ui/assistant-ui) | 自有轻组件 | MIT | 包装 Existing/External Runtime，数据只来自 Client SDK | Direct Dependency | 复用消息、流式状态、Tool Call、附件和中断 UI；不让其直接执行后端任务 |
| 表格和台账 | [TanStack Table](https://github.com/TanStack/table) | React Admin Datagrid | MIT | React 依赖 | Direct Dependency | 客户、项目、报价、合同、应收、Finding 和经营台账 |
| 动态业务表单 | [react-jsonschema-form](https://github.com/rjsf-team/react-jsonschema-form) | [JSON Forms](https://github.com/eclipsesource/jsonforms) | Apache-2.0 / MIT | JSON Schema + UI Schema | Direct Dependency | 需求、报价、审批和规则表单；Schema 仍由后端协议管理 |
| 表单设计器对照 | [Formily](https://github.com/alibaba/formily) | [Form.io.js](https://github.com/formio/formio.js) | MIT | 只做复杂配置页 PoC | Conditional Direct Dependency | 默认仍用 RJSF；只有规则/模板管理员确实需要可视化搭表单时再引入 |
| 条款草稿与批注编辑 | [Lexical](https://github.com/facebook/lexical) | [Tiptap](https://github.com/ueberdosis/tiptap) | MIT | React headless editor | Conditional Direct Dependency | 只编辑审查意见、条款草稿和批注；正式 DOCX 仍由 Office sidecar 生成 |
| 大列表和高密度台账 | [React Virtuoso](https://github.com/petyosi/react-virtuoso) | [Glide Data Grid](https://github.com/glideapps/glide-data-grid) / [React Data Grid](https://github.com/Comcast/react-data-grid) | MIT / MIT / MIT | React 可见区域渲染 PoC | Conditional Direct Dependency | 500+ 行或复杂单元格出现性能瓶颈时采用；普通页面继续 TanStack Table |
| CRM 页面骨架 | [React Admin](https://github.com/marmelab/react-admin) + [Atomic CRM](https://github.com/marmelab/atomic-crm) | [Refine](https://github.com/refinedev/refine) | MIT | 用 Client SDK 实现 Data Provider 的 PoC | Conditional Direct Dependency | 若 PoC 能保持现有协议和视觉自由度则采用；否则只借记录页和筛选 UX |
| 复杂可视状态 | [XState](https://github.com/statelyai/xstate) | 自有 reducer | MIT | 仅前端可视状态 | Direct Dependency（按需） | 后端状态机仍是业务权威 |
| 待处理收件箱/审批 | [Paperclip](https://github.com/paperclipai/paperclip) | OpenSwarm | MIT | 不整体集成 | Design Reference Only | 借 Attention Inbox、blocked task、approval、activity、permission snapshot |
| 多 Agent 空间交互 | [OpenSwarm](https://github.com/openswarm-ai/openswarm) | Paperclip | MIT | 不整体集成 | Design Reference Only | 只借 Agent 状态卡、全局审批队列和 Artifact 卡；不引入 Electron/FastAPI/Claude Runtime |
| 图表 | [Apache ECharts](https://github.com/apache/echarts) | [Recharts](https://github.com/recharts/recharts) | Apache-2.0 / MIT | React 轻封装 | Direct Dependency | 回款、阶段、风险和负荷看板 |

### 4.1 React Admin PoC 门禁

React Admin/Atomic CRM 只有同时满足以下条件才进入主线：

- Data Provider 只调用 `BsaigcClient`，不直接访问数据库、HTTP 或 Tauri；
- 不形成第二套权限、缓存和业务状态权威；
- 页面可以使用现有视觉系统重写，不被 Material UI/默认布局锁死；
- Event revision、CAS、Approval 和 Task 事件仍能正确投影；
- 打包体积和首屏时间可接受。

否则继续采用 `assistant-ui + RJSF + TanStack Table` 的更轻组合。

## 5. Document Intelligence、OCR 和证据定位

| 能力 | 主选 | 备选 | License | 集成方式 | 复用级别 | PoC 目标 |
|---|---|---|---|---|---|---|
| 复杂合同结构解析 | [Docling](https://github.com/docling-project/docling) | [Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg) | MIT / MIT | Python self-contained sidecar / Rust 对照 | Isolated Sidecar | Docling 主跑复杂 PDF、表格、阅读顺序和 provenance；Kreuzberg 只有在真实样本胜出时替换快速路径 |
| Rust-native 快速路径 | [LiteParse](https://github.com/run-llama/liteparse) | Kreuzberg | Apache-2.0 / MIT | Rust direct | Conditional Direct Dependency | 原生 PDF、简单 Office 和低延迟预览；必须逐格式验证 page/bbox 是否真实填充 |
| 中文扫描件 OCR | [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) | [RapidOCR](https://github.com/RapidAI/RapidOCR) / Tesseract / OCRmyPDF | Apache-2.0 / Apache-2.0 / Apache-2.0 / MPL-2.0 | 同一受控 sidecar 内的可切换 adapter | Isolated Sidecar | PaddleOCR 做质量主线；RapidOCR 验证 ONNX/Windows 轻量包；Tesseract 只做离线保底 |
| PDF bbox 快速对照 | [OpenDataLoader PDF](https://github.com/opendataloader-project/opendataloader-pdf) fast | LiteParse | Apache-2.0（2.0 前为 MPL-2.0） | Java 21 sidecar PoC | Isolated Sidecar（条件） | 只测试 born-digital PDF 的 page+bbox；若 JRE 体积收益不成比例则淘汰 |
| 重模型 OCR/布局对照 | [docTR](https://github.com/mindee/doctr) | [Surya](https://github.com/datalab-to/surya) / [olmOCR](https://github.com/allenai/olmocr) | Apache-2.0；模型权重另审 | 不进入主链 | Design Reference Only / Reject | docTR 只做质量基准；Surya 权重含非商业/研究限制，默认阻断；olmOCR 偏 GPU 数据集流水线，不进桌面主链 |
| 通用格式兜底 | [Microsoft MarkItDown](https://github.com/microsoft/markitdown) | Apache Tika | MIT / Apache-2.0 | sidecar | Isolated Sidecar | 不支持格式至少能转为可审计文本；不能替代坐标级 Evidence |
| 长文档结构化提取 | [LangExtract](https://github.com/google/langextract) | 自有 JSON Schema Tool | Apache-2.0 | 先做小型 sidecar PoC | Conditional Direct Dependency | 字段值必须携带原文 span/page/block 引用 |
| PII/敏感信息脱敏 | [Microsoft Presidio](https://github.com/microsoft/presidio) | 自有规则 | MIT | Python sidecar | Isolated Sidecar | 日志、诊断、模型请求前可配置脱敏 |
| PDF 预览 | [PDF.js](https://github.com/mozilla/pdf.js) | react-pdf | Apache-2.0 / MIT | React 依赖 | Direct Dependency | 页码、文本层、缩放、可见页渲染 |
| Evidence 高亮 | [react-pdf-highlighter](https://github.com/agentcooper/react-pdf-highlighter) | PDF.js 自有 overlay | MIT | 选择性封装 | Selective MIT Reuse | Finding 点击后跳页并高亮 bbox/text span |
| 合同审查交互 | [OpenContracts](https://github.com/Open-Source-Legal/OpenContracts) | Paperless/Mayan | MIT | 不整体启动 Django/Postgres/Celery | Design Reference + Selective MIT Reuse | 借文档/字段双栏、批准/拒绝、Annotation/Relationship、引用回答 |
| 文档归档体验 | [Paperless-ngx](https://github.com/paperless-ngx/paperless-ngx) | [Mayan EDMS](https://github.com/mayan-edms/Mayan-EDMS) / Docspell | GPL-3.0 / Apache-2.0 / AGPL | 不作为权威服务 | Design Reference Only | 标签、对应方、类型、版本、消费状态和归档搜索 |

### 5.1 Document Intelligence 统一输出

所有 parser/OCR 必须适配成同一个 BSAIGC 协议，而不是把第三方 DTO 暴露给 UI：

```text
DocumentExtraction
├─ documentId / assetId / documentSha256
├─ parserName / parserVersion / schemaVersion
├─ ocrEngine / modelVersion
├─ pages[]: pageIndex, displayPageNo, width, height, rotation, cropBox, mediaBox
├─ blocks[]: stableElementId, type, text, page, bbox/polygon, order, confidence
├─ tables[]: cells, rowSpan, colSpan, page, bbox
├─ coordinateOrigin / coordinateUnit
├─ metadata / warnings[]
└─ pageRenderHash[]
```

Finding 只能引用稳定 Evidence：

```text
EvidenceRef
├─ documentId / documentSha256
├─ parserVersion / modelVersion / schemaVersion
├─ pageIndex / displayPageNo
├─ stableElementId / tableCellId
├─ textSpan / quoteDigest
├─ bbox / polygon
├─ coordinateOrigin / coordinateUnit
└─ pageRenderHash
```

## 6. Office 模板、生成、对比和 PDF

### 6.1 主选：.NET self-contained Office sidecar

| 组件 | License | 用途 | 复用级别 |
|---|---|---|---|
| [Open XML SDK](https://github.com/dotnet/Open-XML-SDK) | MIT | 读取、修改和保存真实 DOCX/XLSX 包，保留原模板结构 | Direct Dependency（Sidecar 内） |
| [Open-Xml-PowerTools](https://github.com/OpenXmlDev/Open-Xml-PowerTools) | MIT | DOCX 对比、修订、内容操作和 WmlComparer | Direct Dependency（Sidecar 内） |
| [ClosedXML](https://github.com/ClosedXML/ClosedXML) | MIT | XLSX 单元格、表格、命名区域和格式处理 | Direct Dependency（Sidecar 内） |
| [MiniWord](https://github.com/mini-software/MiniWord) | Apache-2.0 | Word 占位符、循环和表格模板填充 | Direct Dependency（Sidecar 内） |

sidecar 只接受纯 JSON 和 Vault 中受控临时输入，返回 Artifact manifest：

```text
OfficeCommand
├─ operation: inspect | fill | compare | render-request
├─ templateAssetId
├─ inputAssetIds[]
├─ fields
├─ outputFormat
├─ idempotencyKey
└─ deadlineMs
```

禁止把 Windows 绝对路径、模板脚本能力或 Office 凭据发给 Renderer。

### 6.2 Rust 和转换补充

| 能力 | 主选 | 备选 | License | 集成方式 | 结论 |
|---|---|---|---|---|---|
| 新建报价 XLSX | [rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter) | ClosedXML | MIT/Apache-2.0 | Rust direct | 适合新建；不能修改已有工作簿 |
| 读取 XLS/XLSX/ODS | [Calamine](https://github.com/tafia/calamine) | ClosedXML | MIT | Rust direct | 读取值和表，不承诺完全格式保真 |
| 稳定 PDF 报告 | [Typst](https://github.com/typst/typst) | PDFMake | Apache-2.0 | CLI sidecar 优先 | 审查报告、归档报告和清单主选 |
| Office→PDF | LibreOffice headless | 商业转换 SDK | MPL-2.0 等混合组件 | 隔离 sidecar，人工审批 | 只做转换，不作为模板/业务引擎 |
| PDF 拆分/合并/校验 | [qpdf](https://github.com/qpdf/qpdf) | [pdfcpu](https://github.com/pdfcpu/pdfcpu) | Apache-2.0 | CLI sidecar | 二选一主装，避免重复打包 |
| DOCX 浏览器预览 | [docxjs](https://github.com/VolodymyrBaydalka/docxjs) | [Mammoth.js](https://github.com/mwilliamson/mammoth.js) | Apache-2.0 / BSD-2 | React 可选 | 只作轻预览；正式打印以 PDF Artifact 为准 |
| XLSX 浏览器编辑 | [Univer](https://github.com/dream-num/univer) | 自有表格 | Apache-2.0 | 后续 PoC | 1.0 默认不嵌入完整电子表格，防止前端变重 |

### 6.3 明确淘汰

- `docx-templates`：MIT，但模板可执行 JavaScript；不作为默认公司模板引擎；
- Carbone：自定义 CCL，商用前需要单独法务审查；
- PyMuPDF：AGPL 或商业授权，不作为默认 PDF 引擎；
- diff-pdf：GPL-2.0 且维护活跃度不足，不进入默认发行包；
- MinerU：附加商业阈值条款，不进入默认依赖；
- Stirling-PDF：根 MIT 但包含 proprietary/saas/engine 等分区许可，且 Java 服务过重，桌面 1.0 暂不采用；
- PhpSpreadsheet / PHPWord：功能成熟，但新增 PHP runtime，且 PHPWord 为 LGPL-3.0；现有 .NET Office sidecar 更匹配；
- SheetJS：GitHub 镜像已不再是主要开发入口，且当前 Office 主路径不需要再引入一套 JS 工作簿权威；
- PptxGenJS：MIT 且成熟，但 PPT 不在当前商务 1.0 范围，保留给后续 Artifact 模块。

## 7. 规则、权限、任务、检索和诊断

| 能力 | 主选 | 备选 | License | 集成方式 | 决策 |
|---|---|---|---|---|---|
| 确定性规则 | [GoRules Zen](https://github.com/gorules/zen) | json-rules-engine | MIT / ISC | Rust crate/API PoC | 通过则用于审批阈值、金额校验、条款规则；规则执行结果必须可解释 |
| 权限 | 现有 Security + [Cedar](https://github.com/cedar-policy/cedar) PoC | Casbin-rs | Apache-2.0 | Rust direct | Cedar 只评估资源级授权表达力；现有后端仍是权限权威，策略结果必须可审计 |
| 外部策略服务 | [OPA](https://github.com/open-policy-agent/opa) | [SpiceDB](https://github.com/authzed/spicedb) | Apache-2.0 | 不采用独立服务 | 只借 policy/relation 测试思路，不增加端口、数据库或第二权限权威 |
| .NET 规则对照 | [Microsoft RulesEngine](https://github.com/microsoft/RulesEngine) | GoRules Zen | MIT | Office sidecar 内只做 benchmark | 不把商务规则执行权威迁入 .NET；仅验证复杂表达式和规则调试体验 |
| Durable Task | 现有 SQLite Task Ledger | Apalis | 自有 / MIT | 不替换权威 | 借队列/重试模型；Apalis 版本口径稳定后再评估 |
| 权威数据库 | SQLx + SQLite | 无 | MIT/Apache + Public Domain | Rust direct | 继续使用 |
| 全文检索 | SQLite FTS5 + [Tantivy](https://github.com/quickwit-oss/tantivy) | 无 | MIT | Rust direct | FTS5 做最小路径；Tantivy 做大量文档索引 |
| 本地向量检索 | [sqlite-vec](https://github.com/asg017/sqlite-vec) | [LanceDB](https://github.com/lancedb/lancedb) / [USearch](https://github.com/unum-cloud/USearch) | Apache-2.0 | Native extension / embedded library PoC | 先验证 Windows 打包、备份、迁移和崩溃恢复；失败则 1.0 只上全文检索 |
| 本地 embedding | Provider embedding | [FastEmbed](https://github.com/qdrant/fastembed) | Apache-2.0 | Python sidecar benchmark | 1.0 非阻塞；模型权重、体积、CPU 延迟和许可证必须单独验收 |
| Agent/规则回归评测 | [promptfoo](https://github.com/promptfoo/promptfoo) | [DeepEval](https://github.com/confident-ai/deepeval) | MIT / Apache-2.0 | 仅开发/CI 工具 | 用匿名 fixture 测漏报、误报、结构化输出和模型切换；不进入产品 Runtime |
| 本地备份与加密导出 | [restic](https://github.com/restic/restic) + [age](https://github.com/FiloSottile/age) | 自有 ZIP/manifest | BSD-2-Clause / BSD-3-Clause | 受控 CLI sidecar PoC | Host 仍维护备份 manifest、恢复演练和权限；CLI 只处理数据块与加密 |
| 只读分析引擎 | SQLite 查询 | [DuckDB](https://github.com/duckdb/duckdb) | MIT | 暂不引入 | 经营看板先用 SQLite；只有大批量只读分析出现真实瓶颈才评估，禁止成为第二业务权威 |
| 独立向量服务 | Qdrant | Meilisearch | Apache-2.0 / MIT+BUSL | 不采用 | 桌面 1.0 不增加 server、端口和第二数据目录 |
| 结构校验 | Ajv / Zod | Rust serde 校验 | MIT | Client SDK 边界可选 | 只做防御性输入校验，协议 source of truth 仍是 Rust |
| 诊断追踪 | [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust) | 现有结构化日志 | Apache-2.0 | Rust direct | traceId、taskId、toolCallId 贯通；默认本地，脱敏后入 outbox |

### 7.1 第二轮新增 36 个成熟候选的处置

第二轮不是继续堆 Star 清单，而是补齐首轮缺的授权策略、复杂表单/编辑器、企业流程、轻量 OCR、评测、备份和本地分析。结论如下：

| 分组 | 新增项目 | 当前处置 |
|---|---|---|
| Policy/Rule | Microsoft RulesEngine、OPA、Cedar、SpiceDB | Cedar 进 Rust PoC；RulesEngine 只 benchmark；OPA/SpiceDB 只借测试模型 |
| UI/Form/Editor/Grid | AG Grid、React Data Grid、Glide Data Grid、React Virtuoso、Form.io.js、Formily、Lexical、Tiptap | Lexical/Tiptap、Formily、虚拟列表按需 PoC；AG Grid 因双许可证不作默认主选 |
| 业务/流程 | docassemble、Apache OFBiz、Bigcapital、Lago、metasfresh、iDempiere、Node-RED、Trigger.dev、Kestra | 全部只借领域、访谈、审批和失败状态；不引入第二业务/任务平台 |
| OCR/Document | RapidOCR、docTR、Surya、olmOCR | RapidOCR 进入 OCR PoC；docTR 做质量基准；Surya 权重许可默认阻断；olmOCR 不适合桌面主链 |
| Office | PhpSpreadsheet、PHPWord、SheetJS、PptxGenJS | 当前不采用；避免新增 PHP/JS Office 权威，PPT 延后 |
| Search/Eval/Data/Backup | USearch、FastEmbed、promptfoo、DeepEval、DuckDB、restic、age | promptfoo 进开发回归；restic/age 进备份 PoC；USearch/FastEmbed 条件评估；DuckDB 延后 |

### 7.2 成熟项目进入主线的硬门槛

候选项目必须同时满足：

1. 能锁定版本、commit、顶层和嵌套许可证；模型权重另行登记；
2. Windows 11 离线环境可安装，不依赖用户全局 Python/Java/.NET/PATH；
3. 能通过纯 JSON、文件 Artifact 或稳定 Rust/JS API 接入，不绕过 Client SDK；
4. 不形成第二套 Task、Approval、Agent、权限或业务数据权威；
5. 真实中文合同、真实 Office 模板和 500+ 记录 UI 压测胜过现有方案；
6. 失败可取消、重试、恢复并产生可诊断日志；
7. 安装体积、冷启动、内存和升级迁移收益大于集成成本；
8. PoC 失败即淘汰，不因 Star 数量或已投入时间硬上。
## 8. 许可证与供应链门禁

### 8.1 默认准入

```text
自动准入：MIT / BSD / Apache-2.0 / ISC / Zlib / 0BSD
人工审批：MPL / LGPL / EPL / CDDL / 多重许可证 / LicenseRef
发行阻断：GPL / AGPL / SSPL / BSL / BUSL / Elastic / Commons Clause /
          Non-Commercial / Research-Only / Unknown / Enterprise
```

“发行阻断”不是永远不能运行，而是未经人工和法务批准不能进入闭源安装包。

### 8.2 每个依赖必须登记

```text
componentName
repository
exactCommit / version
rootLicense
nestedLicenses
noticeFiles
copyrightFiles
distributionMode
modified
bundled
runtimeDownloaded
modelArtifacts
approvalResult
reviewDate
```

### 8.3 正式构建产物

```text
THIRD_PARTY_NOTICES.txt
licenses/
sbom.spdx.json
sbom.cyclonedx.json
model-manifest.json
build-provenance.json
sidecar-manifest.json
```

## 9. 四个先行 PoC

大规模业务开发前先完成四个有淘汰结论的真实 PoC，避免选错底座后返工。

### POC-01 中文合同解析和 Evidence

输入：

- 2 份原生中文 PDF；
- 2 份真实 DOCX；
- 1 份复杂表格合同。

对比：Docling 主线、LiteParse Rust 对照；仅在结果不达标时追加 Kreuzberg 或 OpenDataLoader fast。

验收：

1. 标题、段落、表格、页码和阅读顺序可用；
2. 金额、日期、主体、付款和验收条款能追溯到 page/block/bbox；
3. 输出可序列化为统一 `DocumentExtraction`；
4. 30MB 文件不会压垮 WebView；
5. 失败可取消、重试并保留诊断。

### POC-02 中文扫描 PDF OCR

输入：2 份 300dpi 中文扫描合同和 1 份倾斜/有印章样本。

对比：PaddleOCR 主线、RapidOCR Windows/ONNX 轻量对照，Tesseract/OCRmyPDF 只做 fallback。

验收：

1. 300dpi 正文 CER 目标不高于 3%，金额、日期、公司名称、合同编号等关键字段 exact match 目标不低于 95%；
2. 每个 token/line 有页码与 bbox，随机 200 个 Anchor 在 PDF.js 中页命中率 100%、高亮正确率目标不低于 99%；
3. 印章、倾斜和低清晰度产生明确 warning；
4. OCR 模型、字典和运行时可离线打包并单独登记许可证；
5. 不把原始文件或识别文本上传第三方。

### POC-03 真实 DOCX/XLSX 模板保真

输入：公司报价 XLSX、合同 DOCX、请款 DOCX、验收 DOCX 各 1-2 份。

实现：Open XML SDK + PowerTools + ClosedXML + MiniWord sidecar。

验收：

1. 占位符、表格循环、图片、页眉页脚和格式可保留；
2. 生成文档可被 Microsoft Office 正常打开；
3. DOCX 原版和修订版可生成结构化差异或 track changes；
4. XLSX 公式、格式、打印区域和命名区域不被破坏；
5. 输出先写 Vault，再返回 `assetId`；重复命令不重复生成。

### POC-04 Agent UI 接入现有协议

实现：assistant-ui + Tool Call/Approval；并用 React Admin/Atomic CRM 做一个客户/项目记录页 PoC。

验收：

1. Renderer 不直接 `invoke`、不 spawn Codex；
2. assistant-ui 使用 Client SDK external store/runtime；
3. Tool running/blocked/approval/completed/replayed 状态能正确恢复；
4. CRM Data Provider 只调用 Client SDK；
5. React Admin 若造成第二套状态权威、视觉锁死或明显体积问题，立即淘汰，只保留 TanStack Table + RJSF。

## 10. 对工期的影响

开源复用减少的是通用能力代码量，不会消灭业务规则、协议适配、真实模板、证据准确性和回归测试。

| 项目 | 原估算 | 复用后目标 | 说明 |
|---|---:|---:|---|
| Document Intelligence | 4-6 日 | 3-5 日 | 解析/OCR 不自研，但统一 Evidence adapter 和真实样本仍需完成 |
| Office 模板与对比 | 3-5 日 | 2-4 日 | 直接采用 Open XML 生态，不从 OPC/DOCX 格式开始写 |
| Agent/商务 UI | 5-7 日 | 4-6 日 | 复用聊天、表格、表单；最终视觉仍需业务试用后调整 |
| 规则执行 | 分散在各模块 | 1-2 日 PoC + 规则编写 | 引擎可复用，公司规则不可复用 |
| 本地备份/加密导出 | 1-2 日 | 0.5-1.5 日 | restic/age 只复用数据块、校验和与加密能力，恢复演练仍由 Host 管理 |
| Agent/规则回归评测 | 1-2 日 | 1 日起步 | promptfoo/DeepEval 只做开发回归，不进入产品 Runtime |
| 总体 1.0 | 25-30 工作日 | **23-29 工作日** | 4 个 PoC 可并行；若真实资料或模板延迟，仍按 25-30 日 |

不要用“开源项目很多”虚假压缩时间。真正可节省的主要是：

- PDF/DOCX/XLSX 格式底层；
- OCR 和布局解析；
- Agent 消息/Tool Call 组件；
- 动态表单、数据表格和图表；
- Office 模板与 DOCX 对比；
- 规则引擎、搜索、诊断和许可证工具链。

仍然必须自己完成：

- BSAIGC Command/Event/Task/Asset/Memory 协议；
- 商务唯一主数据和状态机；
- Codex Tool Registry、权限、Approval 和 durable recovery；
- 公司合同规则、报价规则和审批规则；
- 第三方结果到 Evidence/Artifact 的 adapter；
- 真实模板映射、业务 UI 组合和回归验收。

## 11. 实施顺序

```text
第 1-3 日
  POC-01 / 02 / 03 / 04 并行
  同时冻结 OSS 版本、许可证和 adapter 协议

第 3-6 日
  Codex Tool Dispatch
  DocumentExtraction / Evidence / Finding 协议
  Office sidecar skeleton

第 6-12 日
  合同审查 Alpha
  真实 OCR/解析和 Evidence 定位
  审查报告 / 修改建议 / Approval

第 12-20 日
  智能录入、报价、合同、请款、验收、回款
  真实模板第一版
  商务工作台功能 UI

第 20-24 日
  标书、合同版本、Inbox、经营台账、归档、导出

第 24-29 日
  真实同事使用、误报漏报修正、安装包、sidecar、SBOM、签名
```

## 12. 最终选择原则

1. 依赖优先于复制源码；
2. sidecar 优先于把 Python/Java/.NET 运行时塞进 React；
3. 统一 adapter 优先于让每个业务模块直接碰第三方库；
4. 真实样本 PoC 优先于 star 数量；
5. 本地 Task Ledger、Vault 和 SQLite 永远是权威；
6. Codex app-server 永远是唯一 Agent Runtime；
7. GPL/AGPL/商业混合项目只借业务思想，不污染闭源仓库；
8. 每个候选必须有明确淘汰条件，不能因为已经投入时间就硬上。
