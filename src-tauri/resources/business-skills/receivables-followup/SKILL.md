---
name: receivables-followup
description: 根据合同付款节点、请款记录、验收状态和到账记录生成应收账龄、到期提醒、跟进计划和客户沟通草稿，不自动发送。
---

# 应收与回款跟进

## 触发条件
- 查询客户或项目待回款金额
- 付款节点即将到期或已经逾期
- 需要生成催款计划、内部提醒或客户沟通草稿
- 到账记录与合同、请款或验收状态不一致

## 输入
- 合同金额和付款节点
- 请款、验收、到账及撤销记录
- 客户联系人和历史沟通摘要（如已授权）

## 执行步骤
1. 从项目和台账读取合同额、应请款、已请款、已到账、待回款及付款条件。
2. 用计算工具生成逐笔余额、到期日、逾期天数和账龄分组，并写明口径。
3. 检查重复到账、撤销记录、金额不匹配、未验收却请款等异常。
4. 按优先级生成今日/本周跟进计划，区分内部动作和客户动作。
5. 生成礼貌、明确、可编辑的提醒草稿，引用真实项目和单据，不自动发送。

## 人工确认边界
- 到账认领、金额核销、坏账判断、延期承诺和任何外发提醒必须人工确认。
- 未找到银行或财务凭据时只标记“待核”，不得推定到账。

## 输出 Artifact
- `business.receivable.summary`
- `business.receivable.aging`
- `business.receivable.followup-plan`
- `business.receivable.reminder-draft`

## 禁止
- 不自动向客户发送消息。
- 不把经营台账描述成法定会计总账。
- 不删除或覆盖历史到账、撤销和跟进记录。

## 所需工具与权限

工具：
- `business.project.read`
- `business.ledger.read`
- `business.calculation`
- `business.template.read`
- `business.artifact.create`
- `business.approval.request`

权限：
- `project.read`
- `ledger.read`
- `artifact.write:draft`
