---
name: payment-request-pack
description: 引用项目主档和合同付款节点生成请款申请、金额核对和附件清单。用于首款、进度款、尾款或补充请款材料准备。
---

# 请款材料生成

## 触发条件
- 达到合同付款节点
- 需要生成请款申请或开票信息
- 客户要求补充请款附件
- 核对累计请款与剩余金额

## 输入
- 项目主档当前 revision
- 合同及付款条款
- 已请款/已回款记录
- 开票资料与本次节点证明

## 执行步骤
1. 确认本次付款节点、触发条件、应请金额和币种。
2. 用计算工具核对合同总额、累计已请、累计已收、本次请款和剩余金额。
3. 检查发票、验收/进度证明、账号信息、签章等附件要求。
4. 生成请款申请草稿、金额核对表和缺失附件清单。

## 人工确认边界
- 本次金额、收款账户、开票信息和外发材料必须人工确认。
- 未满足付款触发条件时只输出风险，不生成“可提交”结论。

## 输出 Artifact
- `business.payment-request.draft`
- `business.payment-request.reconciliation`
- `business.payment-request.attachment-checklist`

## 禁止
- 不得臆造回款、发票、账户、验收或付款条件已满足。
- 不得修改合同金额或历史财务记录。

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
- `finance.read`
- `template.read:payment`
- `artifact.write:draft`
