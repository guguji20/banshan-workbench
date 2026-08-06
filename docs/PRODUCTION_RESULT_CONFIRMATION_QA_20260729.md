# 制作成果确认 v1 QA 与视觉验收基线

> 日期：2026-07-29
> 输出角色：`production-result-confirmation`
> 自动门禁：`scripts/qa-production-result-confirmation.ps1`
> 范围：仅制作成果确认 v1；候选 v2 `（最新）成果确认书.docx` 不在本基线内。

## 1. 冻结输入

| 角色 | 只读文件 | SHA-256 | 基线 |
|---|---|---|---|
| 制作成果确认 v1 模板 | `真实需求/瑞玺AI请款资料/【空白验收模版】/成果确认书（制作类）.docx` | `7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF` | A4 纵向，2 页；第 1 页为主表，第 2 页以“附脚本：”起始 |
| 历史成品 | `真实需求/瑞玺AI请款资料/【以往验收资料】/97520请款7件套/成果确认书（制作类）.pdf` | `BD56B931D02863B9DBC515D764D704F6F61B6D29CB1F0EF9CB86C2AA0A8D5546` | 22 页历史版式基线 |

脚本默认对两个冻结 SHA-256 做硬校验。任一哈希变化都必须停止生成并重新做模板映射、结构审计和逐页视觉回归，不能通过修改预期哈希绕过。

## 2. 历史 22 页基线

- 第 1-6 页：成果确认主表，历史样本展开 3 个交付类别及 6 条视频证据。
- 第 7-22 页：16 页“附脚本”分镜附件。
- 分镜附件共 54 个镜号，按 4 个脚本章节展开。
- 每个镜号允许 1-3 张图片；图片数量来自冻结业务数据，不得从文件名、数组顺序或历史 PDF 猜测。
- 分镜表固定为 `镜号 / 画面 / 画面描述 / 贴屏文案 / 备注` 五列语义结构。
- 章节标题必须出现且顺序正确；同一镜号的文字、图片和备注应保持为一个视觉单元，表头跨页时必须重复。
- 历史 22 页只用于字段、分页和视觉对照，不能反向替代 SQLite 快照、Vault Asset 引用或人工确认数据。

## 3. 允许的大页尾空白

历史 PDF 第 10、14、22 页存在较大页尾空白。其原因是大表格行、同镜号图片组或章节块不允许拆分，属于可接受分页结果。

允许空白必须同时满足：

1. 前一完整镜号或章节块没有被裁切、压扁或拆到下一页；
2. 下一页从完整镜号、重复表头或完整章节标题开始；
3. 没有通过缩小字号、压缩行距、扭曲图片比例或越过页边距来强行消除空白；
4. 最后一页仍包含完整业务内容，不是无内容的尾随空白页。

除上述保护不可拆视觉单元导致的页尾空白外，连续大面积空白、空白中间页和纯空白尾页都不可接受。

## 4. 自动 QA 范围

`qa-production-result-confirmation.ps1` 可重复运行，任一硬断言失败返回非零退出码。脚本只读模板、历史 PDF 和传入产物；未指定输出路径时，Word 导出的 PDF 写入独立临时目录并在验收后清理。

自动检查包括：

- 计算模板、历史 PDF、待验收 DOCX/PDF 和 Word 导出 PDF 的 SHA-256；
- 固定校验模板 SHA-256、历史 PDF SHA-256与历史 PDF 22 页；
- 检查模板和待验收 DOCX 的 ZIP/OOXML 必需条目及重复条目；
- 安全解析全部 XML/RELS，拒绝 DTD、损坏 XML 和无法解析的关系文件；
- 拒绝 VBA 宏、宏 Content Type、宏 relationship、External relationship 和外部 URL 字段代码；
- 拒绝 `{{...}}`、`${...}`、`<<...>>`、`[[...]]`、`<%...%>`、`TODO/TBD/PLACEHOLDER`、`请输入/待填写/待填入` 等未解析占位符；
- 拒绝 `w:highlight`、黄色 `w:shd` 和未接受的插入、删除、移动修订标记；
- 检查 relationship ID、图片 target、`wp:docPr` 和 `pic:cNvPr` ID 唯一性，并确认图片关系指向包内真实文件；
- 检查 PDF 文件头、EOF、可识别页数及指定页数；
- 系统有 Word 时，以只读方式打开 DOCX、导出 PDF、比对 Word/PDF 页数、复核源 DOCX 前后 SHA-256，并清理本次启动的 `WINWORD`；
- QA 结束后复核所有输入文件仍存在且 SHA-256 未改变。

自动检查不能证明文字没有遮挡、图片没有裁切、字号可读、签章留白合理或 54 个镜号业务内容正确。因此自动门禁通过后仍必须执行第 5 节逐页检查。

## 5. 人工逐页视觉验收

### 5.1 主表第 1-6 页

- 标题、附件编号、项目名称、合同名称、供应商和付款金额正确，无历史项目或示例金额残留。
- 动态交付行数量正确；序号、名称、规格、需求数量、实收数量、单位、验收图片和备注对应同一交付项。
- 长合同名、项目名、供应商名和验收说明自然换行，不遮挡边框，不溢出页边距。
- 图片保持原始宽高比，清晰可辨，无裁切、拉伸、重叠、旋转或低分辨率放大。
- 日期与经办人、专业负责人、其他部门、供应商经办人签章区完整，不伪造签字或公章。

### 5.2 附件第 7-22 页

- 共 16 页附件、4 个章节、54 个镜号；章节和镜号连续，无缺失、重复或串章。
- 每个镜号实际展示 1-3 张图片，图片与画面描述、贴屏文案和备注一致。
- 五列表头在跨页后正确重复；章节标题不得单独落在页尾，镜号编号不得与内容分离。
- 同一镜号尽量不跨页；若内容高度确实超过单页，拆分点必须明确且不把图片与其说明分离。
- 第 10、14、22 页允许出现第 3 节定义的大页尾空白，不因此压缩字体或图片。
- 正式版没有黄色高亮、批注、修订气泡、编辑提示、未解析字段或模板示例值。

### 5.3 不可接受问题

以下任一项都应阻断正式交付：

- 冻结模板或历史基线 SHA-256 不匹配；
- DOCX ZIP 损坏、XML 不可解析、宏、外链、未解析占位符、高亮或未接受修订残留；
- 历史基线回归出现缺页、多页、空白中间页或无业务内容的尾随空白页；
- 16 页附件、4 个章节、54 个镜号或每镜 1-3 图约束不满足；
- 图片关系缺失/重复、图片无法显示、宽高比改变、裁切、重叠、越界或与镜号错配；
- 表格断裂、表头未重复、章节标题孤页、镜号与说明/图片分离；
- 文字截断、乱码、字号异常、边框丢失、页眉页脚覆盖正文；
- 历史项目、公司、金额、账户、链接、提取码、签字或公章残留；
- 为追求页数而压缩字体、行距或图片，导致可读性降低；
- Word 导出后源 DOCX 被修改，或本次 QA 留下新的 `WINWORD` 进程。

## 6. 运行命令

仅检查冻结模板和历史 PDF：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\qa-production-result-confirmation.ps1 `
  -TemplatePath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -HistoricalPdfPath $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH
```

检查真实生成 DOCX，并在 Word 可用时只读导出临时 PDF；历史基线回归应传入 22 页：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\qa-production-result-confirmation.ps1 `
  -TemplatePath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -HistoricalPdfPath $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH `
  -ArtifactDocxPath "C:\path\to\production-result-confirmation.docx" `
  -ExpectedArtifactPages 22 `
  -RequireWord
```

同时检查已有 PDF，并保留 Word 导出 PDF：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\qa-production-result-confirmation.ps1 `
  -TemplatePath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -HistoricalPdfPath $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH `
  -ArtifactDocxPath "C:\path\to\production-result-confirmation.docx" `
  -ArtifactPdfPath "C:\path\to\production-result-confirmation.pdf" `
  -ExpectedArtifactPages 22 `
  -WordPdfOutputPath "$env:TEMP\production-result-confirmation-word-qa.pdf" `
  -OverwriteWordPdf
```

无 Word 的机器可传 `-SkipWordExport` 只跑结构门禁；发布机应传 `-RequireWord`，确保无法渲染时直接失败。传入 `-ExpectedArtifactDocxSha256` 可把具体产物哈希加入本次冻结门禁。

## 7. 2026-07-29 自检记录

### 7.1 默认冻结输入

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\qa-production-result-confirmation.ps1 `
  -TemplatePath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -HistoricalPdfPath $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH
```

结果：退出码 `0`，`PASS=31 WARN=0 FAIL=0`。模板 ZIP 共 18 个条目、18 个 XML/RELS、0 个图片关系；历史 PDF 为 22 页。模板和历史 PDF SHA-256 均与冻结值一致，运行前后输入哈希不变。

### 7.2 Word 可选导出

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\qa-production-result-confirmation.ps1 `
  -TemplatePath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -HistoricalPdfPath $env:BSAIGC_ACCEPTANCE_HISTORY_PDF_PATH `
  -ArtifactDocxPath $env:BSAIGC_ACCEPTANCE_TEMPLATE_PATH `
  -ExpectedArtifactPages 2 `
  -RequireWord
```

结果：退出码 `0`，`PASS=63 WARN=0 FAIL=0`。Word 16.0 以只读方式渲染 2 页，导出 PDF 也是 2 页；该次 PDF SHA-256 为 `19203E2EDA268E962E6C3522E2E5BA0A92064855470CB8EE071AA821C0066187`。源模板导出前后 SHA-256 不变，临时 PDF 目录已删除，本次启动的 `WINWORD` 已清理。

### 7.3 失败退出门禁

使用全零值作为故意错误的 `-ExpectedHistoricalPdfSha256` 执行。实际结果为 `PASS=30 WARN=0 FAIL=1`，错误项为“历史 PDF SHA-256 与冻结基线一致”，退出码 `1`。失败门禁符合预期。
