---
name: business-brief-intake
description: 把客户聊天记录、会议纪要、邮件或需求文档整理成结构化商务 Brief，并识别缺失信息与追问项。用于新商机录入、需求模糊、交接前补全或需求变更复核。
---

# 商务需求整理

## 触发条件
- 新客户需求进入
- 会议或聊天记录需要整理
- 需求信息不完整或前后矛盾
- 市场向执行团队交接前

## 输入
- 客户原始材料或可引用文本
- 已知客户/项目名称
- 预算、周期、交付物等已有事实
- 历史版本（如有）

## 执行步骤
1. 提取客户、项目、目标、受众、交付物、数量、规格、预算、周期、审批人和限制条件。
2. 按“已确认/待确认/冲突”标记字段，并为每项保留来源位置。
3. 只追问会阻断报价、排期或合同的关键问题，合并重复问题。
4. 生成 Brief 草稿和变更摘要；等待人工确认后再写入项目主档。

## 人工确认边界
- 客户名称、预算、交付范围、交付日期和最终 Brief 必须由商务确认。
- 不得代替客户作选择；外发追问清单前必须人工确认。

## 输出 Artifact
- `business.brief.draft`
- `business.brief.questions`
- `business.brief.change-summary`

## 禁止
- 不得补造预算、期限、联系人、承诺或交付规格。
- 不得把推测写成已确认事实。

## 所需工具与权限

工具：
- `business.document.extract`
- `business.source.locate`
- `business.artifact.create`
- `business.approval.request`

权限：
- `document.read`
- `project.read`
- `artifact.write:draft`
