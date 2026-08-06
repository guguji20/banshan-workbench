# 验收 DOC 模板映射与历史成品回归基线

> 检查日期：2026-07-29  
> 检查范围：4 个 DOCX、1 个 legacy DOC、5 个同角色历史 PDF  
> 检查方式：原件只读；SHA-256；OOXML/Word 结构读取；legacy DOC 仅在系统临时目录转换；Microsoft Word 导出 PDF；Poppler 144 DPI 渲染；模板 9 页与历史成品 33 页逐页视觉核对  
> 原件保护：外部文件仅通过 `BSAIGC_EXTERNAL_QA_FIXTURE_ROOT` 指定，仓库只记录合成路径和冻结校验结果，未复制客户原件或历史成品

## 1. 结论

1. 合成验收基线“5 个交付文件、6 类内容”的物理关系已经确认：
   - 视频成片验收单：1 类内容。
   - 成果确认书（制作类）：1 类内容。
   - 服务结算清单：1 类内容。
   - 合同结算单 XLSX：1 类内容。
   - 付款申请书 + 合同结算计算表 DOC：同一物理文件承载 2 类内容。
2. 本报告负责 Word 文档侧 4 个物理输出及“成果确认书”候选新版；合同结算单 XLSX 的单元格、公式、打印区和盖章位见 [ACCEPTANCE_XLSX_TEMPLATE_MAP_20260729.md](./ACCEPTANCE_XLSX_TEMPLATE_MAP_20260729.md)。
3. 这些模板都不是可直接变量替换的语义模板：没有稳定内容控件、字段或占位符命名。正式接入必须采用“模板 SHA-256 + 模板版本 + 语义位置映射 + 最终渲染断言”。
4. `成果确认书（制作类）.docx` 是历史 22 页成品的直接版式来源；`（最新）成果确认书.docx` 是结构不同的候选新版，不能在没有客户确认和合格成品回归样本时自动替换旧版。
5. `付款申请书+合同结算计算表.doc` 名称虽位于“空白验收模版”，实际含公司、账户和金额示例，不是空白模板。任何未被覆盖的旧值都可能造成严重错付风险。
6. 历史 PDF 能证明已填充版式，但均未看到实际签字或公章图像，不能作为“已签署法律成品”基线，只能作为排版和字段回归基线。

## 2. 原件与精确 SHA-256

### 2.1 空白模板

| 模板角色 | 原件 | 字节数 | Word 渲染页数 | SHA-256 |
|---|---|---:|---:|---|
| 视频成片验收 | `<EXTERNAL_QA_FIXTURE_ROOT>/templates/synthetic-video-acceptance.docx` | 28,367 | 1 | `CF9E21CEC8C5458F709410A17350B58D066EA98F3E6F15194598EFCFAA38B5FB` |
| 制作成果确认 v1 | `<EXTERNAL_QA_FIXTURE_ROOT>/templates/synthetic-production-confirmation-v1.docx` | 23,409 | 2 | `7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF` |
| 成果确认候选 v2 | `<EXTERNAL_QA_FIXTURE_ROOT>/templates/synthetic-production-confirmation-v2.docx` | 145,282 | 3 | `23D1E5EF6F7188A6C58F1F0931C0FE907F92CBE68A762606238FC0589FEE326C` |
| 服务结算明细 | `<EXTERNAL_QA_FIXTURE_ROOT>/templates/synthetic-service-settlement.docx` | 17,420 | 1 | `022D1399FD4CA8A2A04C006191C9865B79372B8B721B9776C38D0EFAF502FA56` |
| 付款申请 + 合同结算计算 | `<EXTERNAL_QA_FIXTURE_ROOT>/templates/synthetic-payment-application.doc` | 48,128 | 2 | `E1BF122AFDF3EF15017F3D82E9CAB5DA1C8D3BE38FEA40299906EE61538D5072` |

### 2.2 历史成品

| 成品角色 | 原件 | 字节数 | 页数 | SHA-256 |
|---|---|---:|---:|---|
| 视频成片验收 | `<EXTERNAL_QA_FIXTURE_ROOT>/baselines/synthetic-acceptance-package/video-acceptance.pdf` | 699,332 | 7 | `9C79E59A081B5014CA604C24A0DFC76131578586F4AF6D1619FC983F1330745F` |
| 制作成果确认 | `<EXTERNAL_QA_FIXTURE_ROOT>/baselines/synthetic-acceptance-package/production-confirmation.pdf` | 2,385,618 | 22 | `BD56B931D02863B9DBC515D764D704F6F61B6D29CB1F0EF9CB86C2AA0A8D5546` |
| 服务结算明细 | `<EXTERNAL_QA_FIXTURE_ROOT>/baselines/synthetic-acceptance-package/service-settlement.pdf` | 131,299 | 1 | `2F6A47F51F68B5AD94146D21A75D6E39517EBE518AB984128C97A1BE378E917D` |
| 合同结算确认 | `<EXTERNAL_QA_FIXTURE_ROOT>/baselines/synthetic-acceptance-package/contract-settlement.pdf` | 113,970 | 1 | `91C5396582A36D25ACD1C9A0B954FCB85C1BCEE18A2C9320D1701A7C91AFFAE9` |
| 付款申请 + 合同结算计算 | `<EXTERNAL_QA_FIXTURE_ROOT>/baselines/synthetic-acceptance-package/payment-application.pdf` | 194,625 | 2 | `8EC4D43D591F1022FB1F14C4B853A98BB1D7D0A34AAC92E508964D478278F4F9` |

建议把模板原件作为 Vault Document Asset 管理，并将 `sourceSha256` 冻结到输出规格。源 SHA 不匹配时必须停止定点写值，要求重新做模板映射和视觉回归。

## 3. 5 文件 6 类内容落点

| 内容编号 | 稳定输出角色建议 | 格式 | 物理文件 / 内容落点 | 主要输入 |
|---:|---|---|---|---|
| 1 | `video-completion-acceptance` | DOCX | `视频成片验收单`：合同/项目头、按交付组重复的视频说明、网盘或资产引用、截图、验收结论、甲乙签章区 | 合同、交付项、视频元数据、截图、资产引用、完成时间 |
| 2 | `production-result-confirmation` | DOCX | `成果确认书（制作类）`：第一页成果确认主表；其后“附脚本”分镜表和图片附件 | 合同成果要求、供应商、付款金额、交付项、截图、脚本/分镜、验收意见 |
| 3 | `service-settlement-list` | DOCX | `服务结算清单`：按服务类型重复的结算行、是否按要求提供、证明材料、三方验收签字 | 服务条目、使用时间、证明材料、完成状态 |
| 4 | `contract-settlement` | XLSX | `合同结算单.xlsx`：最终结算书工作表；本报告仅用历史 PDF 核对外观，机器映射见同目录 XLSX 报告 | 项目、合同、合同编号、原合同金额、调整、最终结算金额、双方主体 |
| 5 | `payment-application` | DOC 第 1 页 | `付款申请书+合同结算计算表.doc` 第 1 页：付款期间、开票金额、累计产值、应付/代缴、收款账户、公司盖章 | 合同、付款批次、金额、公司批准后的收款账户 |
| 6 | `contract-settlement-calculation` | DOC 第 2 页 | 同一物理 DOC 第 2 页：合同结算计算表、行项、单价、原合同/结算数量金额、累计已付、剩余应付、签章区 | 合同编号、结算行项、数量、单价、累计已付、最终结算金额 |

`（最新）成果确认书.docx` 不是第 7 类内容，只是内容 2 的候选模板版本。

## 4. 视频成片验收单

### 4.1 模板结构

- A4 纵向，1 页，页边距约上/下 25.4 mm、左/右 31.7 mm。
- 正文独立段落：标题、`合同名称：`、底部附注。
- 主表为 `6 行 x 4 列`，大量合并单元格；无图片、无 Word 字段、无内容控件。
- 语义行：

| 行 | 语义 | 当前模板状态 |
|---:|---|---|
| 1 | 项目名称 / 完成时间 | 项目名含示例值，完成时间空 |
| 2 | 视频主题 / 时长 | 空 |
| 3 | 视频截图和视频说明主体 | 单个大空白区域 |
| 4 | 第二组视频主题 / 时长 | 含历史示例值，不能保留为默认业务值 |
| 5 | 验收结论 | 固定“本次事项已完成并通过验收” |
| 6 | 甲方 / 乙方代表签字与日期 | 双栏大面积签章留白 |

### 4.2 历史成品映射

- 历史成品从 1 页扩展为 7 页。
- 第 1-6 页按 3 个服务组展开，每组含 2 条视频：组头字段、服务说明、金额说明、文件/资产引用、两张代表截图。
- 第 7 页只保留验收结论、甲乙签字盖章区和附注。
- 主表边框跨页连续，左侧“视频截图”标签形成跨页视觉锚点。

### 4.3 机器映射建议

| 语义字段 | 目标 | 规则 |
|---|---|---|
| `contractTitle` | 标题下方 `合同名称：` 段落 | 必须来自冻结合同，不从文件名猜测 |
| `projectTitle` | 第 1 行项目名称值区 | 覆盖模板示例值 |
| `completionDate` | 第 1 行完成时间值区 | 正式导出前必须确认 |
| `deliveryGroups[]` | 第 2-4 行之间的动态主体 | 每组生成组头和一个或多个视频子块，不硬编码 3 组或 2 条 |
| `video.title/type/content/duration` | 每个视频子块 | 视频时长必须与资产元数据/人工确认一致 |
| `video.assetReference` | 每个视频子块 | 优先保存 Asset ID 和文件名；外部分享链接只作为可选证据 |
| `video.screenshots[]` | 每个视频子块 | 保持宽高比；不得拉伸；每张图应带来源 Asset ID/SHA |
| `acceptanceConclusion` | 倒数第 2 行 | 必须人工确认后才可进入正式版 |
| `customerSigner/supplierSigner` | 最后一行 | 保留空白，不自动伪造签字或公章 |

分页必须以“一个视频子块”为最小不可拆单元；图片、文件名、链接说明不得跨页断裂。只有全部动态主体完成后，才附加结论和签章页。

## 5. 成果确认书（制作类）v1

### 5.1 模板结构

- A4 纵向，2 页；第 1 页约 20 mm 四边页边距。
- 第 1 页为一个 `11 行 x 17 列` 的高合并表格；第 2 页只有 `附脚本：` 起始段落。
- 无图片、内容控件或 Word 字段；第 1 页全部靠固定表格单元格定位。
- 关键区域：

| 区域 | 字段 |
|---|---|
| 标题 | 附件编号、制作类标识、合同/项目标题 |
| 基础信息 | 项目分类、项目名称、合同名称、本次付款金额 |
| 成果要求 | 合同对成果要求简述 |
| 采购信息 | 供应商名称、本次采购需求时间 |
| 验收说明 | 成果验收情况描述、不合格处罚/新增部分 |
| 动态交付表 | 序号、名称/材质、规格、需求数量、单位、实收数量、验收图片、备注 |
| 日期与签字 | 执行完成日、验收日、经办人、专业负责人、其他部门、供应商经办人 |
| 附件 | `附脚本：` 后追加分镜脚本和图片表 |

### 5.2 历史成品映射

- 第 1-6 页是主成果确认表，展开 3 个交付类别及 6 条视频证据。
- 第 7-22 页是“附脚本”分镜附件，采用 `镜号 / 画面 / 画面描述 / 贴屏文案 / 备注` 五列结构，共 54 个镜号。
- 分镜附件按 4 个脚本章节展开，章节之间保留标题行；每个镜号可以有 1-3 张图片。
- 历史第 10、14、22 页存在较大页尾空白，是大表格行不允许拆分后的结果。生成器可以改善分页，但不能靠压缩字体或裁切图片消除空白。
- 历史脚本页仍有黄色高亮编辑标记。正式输出不得无条件继承这些标记，必须明确选择“保留来源高亮”或“生成干净版”，默认正式版应阻断并要求人工确认。

### 5.3 机器映射建议

| 语义字段 | 目标 | 规则 |
|---|---|---|
| `attachmentLabel` | 标题前附件编号 | 客户配置项，不硬编码“附件六” |
| `contractTitle/projectTitle` | 大标题及基础信息区 | 两个字段分别冻结，不互相代替 |
| `category` | 项目分类复选项 | 只允许从模板允许值映射；未知值转“其他”并人工确认 |
| `paymentAmountCents` | 本次付款金额 | 整数分存储；显示值与付款申请、结算单一致 |
| `contractDeliverableSummary` | 合同对成果要求简述 | 记录来源合同页码/条款定位 |
| `supplierLegalName` | 供应商名称 | 来自公司/合同冻结数据 |
| `procurementPeriod` | 本次采购需求时间 | 与付款申请期间不必相同，必须独立字段 |
| `deliveryItems[]` | 动态交付表 | 每行绑定交付项 ID、数量、单位、证据 Asset ID |
| `storyboards[]` | “附脚本”后 | 每个脚本含标题、规格、形式、时长和镜号数组 |
| `storyboard.shots[]` | 五列表 | 图片数组按比例插入；行高动态；同一镜号尽量不跨页 |
| `signoffRoles[]` | 主表底部 | v1 固定 4 个角色；不得与 v2 角色表混用 |

## 6. （最新）成果确认书：候选 v2

### 6.1 模板结构

- 第 1、2 页为 A4 横向；第 3 页为 A4 纵向空白页。
- 主表为 `14 行 x 13 列`，字段由“项目分类/需求清单”改为“事项分类/投放清单”。
- 相比 v1 新增合同单价、总价、实收单价/总价、增加/减少金额等财务列。
- 签字角色扩展为经办人、片区负责人、项目营销负责人、专业负责人、物业/合约等其他部门、供应商经办人。
- 没有 `附脚本：` 锚点，无法直接复用 v1 的 16 页脚本附件拼接规则。

### 6.2 版本关系判定

| 证据 | 判定 |
|---|---|
| 文件名含“最新”，OOXML 创建时间晚于 v1 | 支持其为候选新模板 |
| 页面方向、字段名、列数、审批角色均不同 | 不是 v1 的小修订，必须作为独立模板版本 |
| 历史 22 页成品版式与 v1 一致 | 历史成品不能作为 v2 的像素回归样本 |
| v2 第 3 页完整空白 | 当前文件存在分页/节设置风险，不宜直接发布 |
| 没有客户批准记录或合格填充成品 | 不能仅凭文件名把 v2 设为默认 |

建议暂定：

```text
production-result-confirmation.v1 = 成果确认书（制作类）.docx
production-result-confirmation.v2-candidate = （最新）成果确认书.docx
```

只有取得 v2 的适用范围、审批角色规则和一份合格填充成品后，才能将其从 candidate 提升为 active。

## 7. 服务结算清单

### 7.1 模板结构

- A4 纵向 1 页，主表 `3 行 x 7 列`。
- 第 1 行：序号、视频名称、使用时间、服务说明、是否按要求提供、证明材料、备注。
- 第 2 行：单个示例服务行，默认勾选“是”、证明材料为盖章版文件、备注为已完成。
- 第 3 行：乙方验收人、甲方验收人、甲方验收监管人签字与日期。
- 表后为三类验收职责固定说明。

### 7.2 历史成品映射

- 历史成品仍为 1 页，将单个示例服务行扩展为 3 行。
- 三个服务类别与成果确认书、合同结算计算表的行项一致。
- 签字区和职责说明均完整保留，无文本裁切或跨页。

### 7.3 机器映射建议

| 字段 | 目标 | 规则 |
|---|---|---|
| `serviceItems[]` | 表格中间动态行 | 克隆样式行，不复制示例业务值 |
| `serviceName/period/description` | 第 2-4 列 | 与交付项及合同周期一致 |
| `providedAsRequired` | 第 5 列 | 布尔值；未确认时不能默认勾“是” |
| `evidenceLabel/evidenceAssetIds` | 第 6 列 | 显示短说明，完整证据留在冻结 manifest |
| `remarks` | 第 7 列 | “已完成”只能由验收结论确认后写入 |
| `signoffRoles` | 最后一行 | 固定三方角色；保留人工签字与日期空间 |

为保持 1 页，服务行过多时不能无限缩小字号。超过经视觉验证的行数后，应采用续页表并重复表头，签字区只放在最后一页。

## 8. 付款申请书 + 合同结算计算表

### 8.1 模板结构

- 原件是 legacy `.doc`，本次只读打开，并仅在系统临时目录转换为 DOCX 供结构分析；原件未回写。
- A4 纵向 2 页，显式分页。
- 第 1 页“付款申请书”：标题、甲方称呼、合同与完成说明、付款期间、开票/累计产值、应付/代缴金额、收款账户块、公司盖章和日期。
- 第 2 页“合同结算计算表”：一个 `11 行 x 12 列` 高合并表格，包含项目/合同/合同编号、施工单位、动态结算行、累计已付、剩余应付、乙方盖章、经办人、专业经理。
- Word 表格没有计算公式；所有金额都必须由 Host 使用整数分计算后写入。

### 8.2 非空白与敏感值风险

该模板原件含真实供应商名称、银行信息和历史金额示例。正式生成必须执行“模板旧值清零断言”，不能只覆盖本次用到的字段。

必须覆盖或校验的旧值类型：

- 收款单位、开户行、银行账号、联行号。
- 示例应付款金额、累计已付、剩余应付。
- 项目名称、供应商名称。
- 第 2 页默认单位和行项示例。

银行信息必须来自经过人工确认的公司资料版本；不得从历史 PDF、OCR 或模板残留反向导入。日志、前端协议和联网请求不得包含完整银行账号。

### 8.3 机器映射建议

#### 第 1 页：付款申请书

| 语义字段 | 目标 | 规则 |
|---|---|---|
| `customerLegalName` | 称呼抬头 | 与合同甲方一致 |
| `contractTitle` | 第一段下划线区 | 长标题允许换行，不压缩到不可读 |
| `workSummary` | “目前已经完成”后 | 正式付款前人工确认 |
| `paymentPeriodStart/End` | 付款期间 | 与结算批次独立冻结 |
| `paymentSequence` | 第 N 次付款 | 正整数 |
| `invoiceAmountCents` | 本期开票金额 | 与发票/结算一致性校验 |
| `cumulativeRecognizedAmountCents` | 累计产值 | 不得默认等于本期金额 |
| `payableAmountCents` | 本期应付 | 扣除项后确定性计算 |
| `withheldAmountCents` | 代缴金额 | 无代缴时显示 0，但仍需冻结规则 |
| `bankAccountProfileVersion` | 收款账户块 | 只从公司批准版本渲染；正式导出必须人工确认 |
| `companySeal/date` | 页尾 | 保留空白，不自动嵌章 |

#### 第 2 页：合同结算计算表

| 语义字段 | 目标 | 规则 |
|---|---|---|
| `projectTitle/contractTitle/contractNumber` | 表头 | 三个独立字段 |
| `supplierLegalName` | 施工单位名称 | 与付款申请收款单位交叉校验 |
| `settlementItems[]` | 动态行 | 项目、单位、合同单价、原合同数量/金额、结算数量/金额 |
| `cumulativePaidCents` | “二 累计已付金额” | 来自结算台账，不从模板示例读取 |
| `remainingPayableCents` | “三 剩余应付结算金额” | `结算金额 - 累计已付`，与第 1 页应付金额核对 |
| `supplierSeal/handler/professionalManager` | 页底 | 保留人工签章和日期区 |

## 9. 逐页视觉核对结果

| 文档 | 模板页数 | 历史页数 | 视觉结论 |
|---|---:|---:|---|
| 视频成片验收单 | 1 | 7 | 历史表格跨 7 页连续；3 组/6 条视频、截图和引用均可读；第 7 页签章区完整；无裁切 |
| 成果确认书（制作类）v1 | 2 | 22 | 主表扩展到 6 页，脚本附件 16 页；图片与文字清晰；部分页有大面积页尾空白；存在黄色编辑高亮残留 |
| 成果确认书候选 v2 | 3 | 无 | 横向两页主体可读；第 3 页完全空白；没有合格填充样本，不能做历史像素回归 |
| 服务结算清单 | 1 | 1 | 3 个服务行、签字区和职责说明均在 1 页内，无溢出 |
| 合同结算单 | 另见 XLSX 报告 | 1 | 历史 PDF 单页清晰；签章留白和页脚完整；深度映射由 XLSX 报告负责 |
| 付款申请 + 合同结算计算 | 2 | 2 | 两页字段、金额行和签章区完整；无裁切；历史成品仍未实际签章 |

历史成品中的第三方分享链接和提取码只证明当时交付路径，不能作为长期可用性基线。1.0 应优先引用受控 Asset/R2 对象，外部链接仅作为可选证据，并在正式导出前人工确认是否保留。

## 10. 模板包结构与安全

- 4 个 DOCX 均无宏、无外部链接、无评论；legacy DOC 通过 Word 检查无 VBProject。
- DOCX 保留创建者/最后修改者等个人元数据和自定义 XML。生成客户副本时必须清理个人元数据，但不得修改原件。
- 所有模板均无稳定内容控件和字段；现有书签数量也不足以承担业务映射。
- 不得按“第 N 个空单元格”跨模板版本写值。每个版本必须绑定源 SHA、结构断言和语义位置。
- 生成过程中只处理副本；失败时删除临时副本，不覆盖原模板或已批准版本。

## 11. 建议输出规格

```json
[
  {
    "outputCode": "video-completion-acceptance",
    "format": "docx",
    "templateKey": "company.acceptance.video-completion.v1",
    "sourceSha256": "CF9E21CEC8C5458F709410A17350B58D066EA98F3E6F15194598EFCFAA38B5FB"
  },
  {
    "outputCode": "production-result-confirmation",
    "format": "docx",
    "templateKey": "company.acceptance.production-result.v1",
    "sourceSha256": "7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF"
  },
  {
    "outputCode": "service-settlement-list",
    "format": "docx",
    "templateKey": "company.acceptance.service-settlement.v1",
    "sourceSha256": "022D1399FD4CA8A2A04C006191C9865B79372B8B721B9776C38D0EFAF502FA56"
  },
  {
    "outputCode": "payment-application-settlement-calculation",
    "format": "docx",
    "templateKey": "company.acceptance.payment-and-calculation.v1",
    "sourceSha256": "E1BF122AFDF3EF15017F3D82E9CAB5DA1C8D3BE38FEA40299906EE61538D5072"
  }
]
```

候选 v2 单独登记为 disabled/candidate，不能与 v1 共用 `templateKey`。合同结算 XLSX 规格由同目录 XLSX 报告定义。

## 12. 自动化断言

### 12.1 源与结构

```text
ASSERT source_sha256 matches registered template version
ASSERT original_file remains unchanged
ASSERT generated_copy contains no macros or external links
ASSERT generated_copy contains no unresolved placeholders
ASSERT generated_copy contains no stale project/company/amount/bank example values
ASSERT document role and template version are explicit
```

### 12.2 跨文件一致性

```text
ASSERT projectTitle identical across all 5 outputs
ASSERT contractTitle identical across all 5 outputs
ASSERT contractNumber identical where present
ASSERT supplierLegalName identical across all outputs
ASSERT paymentAmount == settlementAmount == payableAmount when business rules require equality
ASSERT sum(settlementItems.amountCents) == settlementAmountCents
ASSERT remainingPayableCents == settlementAmountCents - cumulativePaidCents
ASSERT each evidence image and script row traces to frozen Asset ID + SHA-256
ASSERT each output freezes acceptanceBatchId + batchRevision + outputSpecId + materialManifest
```

### 12.3 最终渲染

```text
ASSERT video-completion-acceptance has no split image blocks or clipped signature area
ASSERT production-result-confirmation repeats script table headers and preserves image aspect ratios
ASSERT production-result-confirmation contains no unapproved highlight/revision markup
ASSERT service-settlement-list keeps signoff roles together on final page
ASSERT payment-application-settlement-calculation exports exactly 2 pages for the White Goose baseline
ASSERT no blank trailing page unless template version explicitly allows it
ASSERT no actual seal/signature is inserted without independent authorization and audit
```

## 13. 不可自动化或必须人工确认的风险

1. `（最新）成果确认书` 与制作类 v1 的适用关系尚未由业务确认。
2. 成果确认 v2 没有填充成品，且当前带完整空白第 3 页。
3. 历史脚本附件含黄色高亮编辑痕迹，无法仅凭版式判断哪些高亮是审批意见、待修改项或正式内容。
4. 历史 PDF 没有实际签字/公章，不能验证盖章后的遮挡、颜色、尺寸和法律有效性。
5. 外部网盘链接和提取码可能过期或泄露，不得作为唯一交付证据。
6. 付款申请模板包含敏感收款信息和旧金额，正式导出必须由人工确认银行账户、公章、签字和付款金额。
7. 长合同名、长项目名、多于历史数量的交付组、超长脚本和非 16:9 图片会改变分页，必须重新渲染逐页核对。
8. 历史基线只有整元金额；含角分、大额数字和负调整的中文大写及换行规则仍需单独业务样本。
9. 当前“4 组要求、3 组素材、缺 1 组”的 readiness 不能从历史 3 类/6 条视频硬推。批次要求必须来自合同和本次验收配置。

## 14. 接入顺序

1. 在 Vault 注册 5 个真实模板 Asset，记录源 SHA；不把原件提交 Git。
2. 固化 5 个输出规格的 `outputCode/format/templateKey/sourceSha256/requirementIds`。
3. 文档快照冻结 `acceptanceOutputSpecId`、批次 revision、合同/金额数据和素材 manifest。
4. 为每个模板实现独立适配器：克隆模板、定点写值、动态扩行/分页、清理元数据、保存可编辑 DOCX。
5. 先实现 `PrepareAcceptanceDocuments` 幂等创建 5 个草稿，再接真实模板渲染；缺素材允许准备草稿，但继续禁止批准和正式生成。
6. 使用本报告 SHA、页数、字段和视觉断言跑合成验收基线回归；v2 未获批准前只跑结构审计，不进入正式输出。
7. 正式导出前执行人工确认：验收结论、付款金额、银行账户、公章、签字、对外文件名和发送范围。
