---
name: business-consistency-audit
description: 跨 Brief、报价、合同、项目主档、请款和验收执行字段级一致性审计。用于签约前、请款前、验收前、归档前或任一关键资料变更后。
---

# 商务文档一致性审计

## 触发条件
- 关键阶段提交前
- 项目资料发生版本变化
- 金额、税率、交付或付款信息存在冲突
- 需要全链路复核

## 输入
- 参与审计的全部 Artifact 及版本
- 项目主档当前 revision
- 共享一致性字段表
- 允许为空或允许差异的业务规则

## 执行步骤
1. 确认输入版本与缺失文档，禁止混用未标记版本。
2. 按共享字段表比较客户、项目、金额、税率、服务范围、数量、规格、付款、交付、验收和版本。
3. 区分一致、允许差异、冲突、缺失和无法判断。
4. 为每个冲突列出各来源值、定位、影响和建议责任人。
5. 生成审计报告和阻断项；修正后可再次审计并关联前次报告。

## 人工确认边界
- 差异是否可接受、采用哪个值及是否解除阻断必须人工确认。
- 审计通过不等于法律、财务或交付最终批准。

## 输出 Artifact
- `business.consistency-audit.report`
- `business.consistency-audit.blockers`
- `business.consistency-audit.recheck`

## 禁止
- 不得自动选择更“合理”的值覆盖冲突。
- 不得隐藏缺失来源或把无法判断标为一致。

## 所需工具与权限

工具：
- `business.artifact.read`
- `business.artifact.compare`
- `business.source.locate`
- `business.project.read`
- `business.artifact.create`
- `business.approval.request`

权限：
- `project.read`
- `artifact.read`
- `artifact.write:audit`
- `audit.write`
