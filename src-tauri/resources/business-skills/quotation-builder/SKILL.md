---
name: quotation-builder
description: 依据已确认 Brief、项目主档和报价模板生成单条、拆项、月度、年框或框架服务报价草案。用于首次报价、方案调整、价格解释和版本对比。
---

# 报价方案生成

## 触发条件
- 需要新建报价
- 客户要求拆项或多方案报价
- 报价范围、数量或税率变更
- 需要解释报价构成

## 输入
- 已确认 Brief 或项目主档
- 适用报价模板/价格表
- 数量、单价、税率、折扣规则
- 历史报价版本（如有）

## 执行步骤
1. 验证报价所需字段，缺失时先输出阻断项。
2. 选择适用模板，逐项列出服务、数量、单位、单价、小计和说明。
3. 使用计算工具核对未税、税额、含税总额及折扣，不手算关键金额。
4. 对比历史版本并标出增减项、金额变化和原因。
5. 生成报价草案、报价说明和待审批项。

## 人工确认边界
- 单价、折扣、税率、利润敏感项和最终总价必须人工确认。
- 未经审批不得标记“最终版”或对外发送。

## 输出 Artifact
- `business.quotation.draft`
- `business.quotation.explanation`
- `business.quotation.diff`

## 禁止
- 不得臆造价格、折扣、税率、成本或公司承诺。
- 不得把模板价格当成已审批价格。

## 所需工具与权限

工具：
- `business.project.read`
- `business.template.read`
- `business.calculation`
- `business.artifact.create`
- `business.approval.request`

权限：
- `project.read`
- `template.read:quotation`
- `artifact.write:draft`
- `pricing.read`
