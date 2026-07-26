# 商务模板字段目录

真实公司模板接入时使用稳定字段名，不让 DOCX/XLSX 直接依赖 SQLite 列名或 React 状态。

## 内置模板键

```text
builtin.quote.standard.v1
builtin.contract.service.v1
builtin.payment-request.standard.v1
builtin.acceptance.standard.v1
```

## 项目字段

```text
project.title
project.code
project.serviceStart
project.serviceEnd
project.deliverySummary
project.paymentTerms
project.acceptanceTerms
project.notes
```

## 客户字段

```text
customer.name
customer.legalName
customer.taxId
customer.address
customer.contact
customer.phone
customer.email
```

## 供应方字段

```text
supplier.legalName
supplier.taxId
supplier.address
supplier.contact
supplier.phone
supplier.bankName
supplier.bankAccount
```

## 金额字段

```text
money.currency
money.defaultTaxRate
money.subtotal
money.tax
money.total
money.totalUppercase
```

金额在协议和数据库中使用整数分，模板执行器负责格式化，不允许模板自行进行浮点计算。
`money.total` / `money.totalUppercase` 表示项目或合同整体金额，只用于报价、合同和验收等项目级单据，不作为请款单的本期应付金额。
当前内置 renderer 的金额大写使用英文格式；公司模板 adapter 可按财务规范替换为人民币中文大写，整数分数据源不变。

## 报价明细

```text
items[].name
items[].description
items[].quantity
items[].unit
items[].unitPrice
items[].taxRate
items[].amount
```

## 单据字段

```text
document.number
document.title
document.kind
document.sequence
document.createdAt
document.approvedAt
document.approvedBy
```

## 请款快照字段

仅 `paymentRequest` 使用；值来自创建单据时冻结的 `BusinessPaymentRecord`，不会随之后的到账状态变化。`payment.amount` / `payment.amountUppercase` 是请款单唯一的本期应付金额来源，不读取项目级 `money.total`。

```text
payment.id
payment.label
payment.amount
payment.amountUppercase
payment.dueAt
payment.reference
payment.statusAtSnapshot
```

## 历史工作区预填与字段来源

`prefillSourceWorkspaceId` 是 Workspace 级审计字段，不是商务模板字段，不应出现在客户可见文档中。

历史 Workspace 只可为新 Workspace 提供以下固定 15 个字段：

| Preview field | Workspace / template value |
|---|---|
| `customerLegalName` | `customer.legalName` |
| `customerTaxId` | `customer.taxId` |
| `customerAddress` | `customer.address` |
| `customerContact` | `customer.contact` |
| `customerPhone` | `customer.phone` |
| `customerEmail` | `customer.email` |
| `supplierLegalName` | `supplier.legalName` |
| `supplierTaxId` | `supplier.taxId` |
| `supplierAddress` | `supplier.address` |
| `supplierContact` | `supplier.contact` |
| `supplierPhone` | `supplier.phone` |
| `supplierBankName` | `supplier.bankName` |
| `supplierBankAccount` | `supplier.bankAccount` |
| `currency` | `money.currency` |
| `defaultTaxRateBps` | `money.defaultTaxRate` 的 basis-point 来源值 |

`project.*`、`items[]`、`payment.*`、单据字段、状态、revision 和历史快照永不复制。目标 Project 与已确认 Requirement Brief 产生的字段先形成 preview 的 `targetValue`；来源字段形成 `sourceValue`，当前白名单结果形成 `resultValue`。每项还返回 `unchanged`、`filled`、`replaced` 或 `cleared`，该决策必须由 Host 计算，UI 不得自行推导。

preview 只是请求时点说明，不是模板快照、preview token 或 CAS。真正创建 Workspace 时，Host 会重新读取来源、重新校验客户并重新应用这 15 个字段。复制完成后来源变化不会自动修改目标，目标变化也不会反写来源。

候选查询只返回哪些字段已填，不返回税号、银行账号等具体值；完整 `sourceValue` 仅在调用方明确选择单个来源并请求 preview 后返回。每份单据仍只读取创建时冻结的 `BusinessDocumentSnapshot`。后续 Workspace 预填、编辑或来源变更都不能污染已存在的报价、合同、请款或验收文件。

## 模板适配规则

- DOCX 模板优先使用内容控件标签；兼容单个 XML text run 内的 `{{field.name}}` 占位符。
- XLSX 模板使用命名单元格和命名表格，不依赖固定行号。
- 缺少必填字段时停止生成并返回结构化缺失项，不生成半成品。
- 模板文件作为 Vault Document Asset 管理，UI 只持有 `assetId` 和 `templateKey`。
- 模板升级必须创建新的 key/version；历史单据继续记录原 `templateKey`。
