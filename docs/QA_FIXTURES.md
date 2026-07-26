# 中文商务 QA 样本

## 用途

`scripts/create-business-qa-fixtures.py` 使用 Python 标准库生成可被 Microsoft Word 和项目现有 DOCX parser 打开的真实 OOXML 文件，用于合同解析、规则审查、Agent 审查、Evidence 定位和回归测试。

全部主体、编号、地址和业务数据均为虚构测试数据，不得作为真实合同使用。

## 生成

在仓库根目录运行：

```powershell
pnpm qa:fixtures
```

输出目录固定为：

```text
.runtime/qa-fixtures/
```

该目录已被 Git 忽略。脚本可重复执行，每次覆盖同名输出，不依赖 PowerShell 中文管道，也不需要第三方 Python 包。

## 样本清单

| 文件 | 预期结果 |
|---|---|
| `01-standard-low-risk-contract.docx` | 字段完整、付款和验收清晰、责任有上限、知识产权边界相对平衡，预期低风险 |
| `02-high-risk-contract.docx` | 包含单方变更、无限责任、付款模糊、验收模糊、知识产权全转让和不利争议地 |
| `03-missing-fields-contract.docx` | 各商务章节存在，但关键值为未填写、待补充或另行约定，用于识别语义缺失而非只检查标题 |
| `manifest.json` | UTF-8 清单，记录每份 DOCX 的 SHA-256、字节数、预期风险、预期缺失字段和验证结果 |
| `04-scanned-contract-ocr.pdf` | 仅包含页面图片、不含文本层的中文扫描合同，用于验证 Windows OCR、取消、Evidence 页码和重启恢复 |
| `04-scanned-contract-ocr.json` | 扫描 PDF 的 SHA-256、字节数和预期 OCR 关键字 |

每份合同都覆盖客户、项目、金额、付款、交付、验收、违约、保密、知识产权和争议解决章节。

## 单次执行内置验证

脚本在写入前完成以下检查，任一检查失败都会以非零状态退出：

1. 每份 DOCX 在内存中独立构建两次，要求字节和 SHA-256 完全一致。
2. ZIP 包必须包含 `[Content_Types].xml`、关系文件、核心属性、`word/document.xml`、样式和设置。
3. ZIP CRC 必须通过，全部 XML 与 RELS 必须可解析。
4. 包级 `officeDocument` 关系和 Word 主文档根元素必须正确。
5. 提取出的正文必须保留中文，不得含 ASCII 问号替代字符或 Unicode replacement character。
6. 所有必需商务章节和每份样本的关键证据文本必须存在。

为保证可重复性，OOXML 文档属性时间固定为 `2026-07-21T00:00:00Z`，ZIP 条目时间固定为 `1980-01-01T00:00:00`，并使用固定顺序和 Deflate level 9 写包。
## 扫描 PDF 生成说明

`scripts/create-scanned-pdf-qa-fixture.ps1` 使用本机 Microsoft Edge 或 Google Chrome 先渲染固定中文合同页面为 PNG，再把该 PNG 单独封装成 PDF。因此该文件没有可直接提取的文本层，必须走 Native Windows OCR。脚本只在被 Git 忽略的 `.runtime/qa-fixtures/` 中写入样本与清单。
