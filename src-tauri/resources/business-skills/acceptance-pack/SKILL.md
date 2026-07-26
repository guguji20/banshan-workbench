---
name: acceptance-pack
description: 依据合同交付和实际交付记录生成验收单、交付清单、缺失项及签章检查。用于阶段验收、终验和客户补充验收材料。
---

# 验收材料生成

## 触发条件
- 项目进入阶段验收或终验
- 需要整理交付物清单
- 客户要求验收证明
- 请款前需要核验验收条件

## 输入
- 项目主档当前 revision
- 合同交付与验收条款
- 实际交付物记录、版本和日期
- 客户确认、链接或文件清单

## 执行步骤
1. 提取合同要求的交付物、数量、规格、格式、期限和验收条件。
2. 逐项匹配实际交付记录，标记已覆盖、部分覆盖、缺失或证据不足。
3. 检查版本、链接/文件标识、交付日期、验收日期、签字盖章和附件。
4. 生成验收单草稿、交付清单及未闭合项。

## 人工确认边界
- 实际交付完成、客户接受、验收日期和签章状态必须人工确认。
- 证据不足时不得标记“验收通过”。

## 输出 Artifact
- `business.acceptance.draft`
- `business.acceptance.delivery-list`
- `business.acceptance.gap-report`

## 禁止
- 不得臆造交付文件、客户确认、版本、日期或签章。
- 不得用计划交付代替实际交付。

## 所需工具与权限

工具：
- `business.project.read`
- `business.artifact.read`
- `business.source.locate`
- `business.template.read`
- `business.artifact.create`
- `business.approval.request`

权限：
- `project.read`
- `delivery.read`
- `template.read:acceptance`
- `artifact.write:draft`
