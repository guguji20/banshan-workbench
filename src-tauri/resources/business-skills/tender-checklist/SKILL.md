---
name: tender-checklist
description: 把采购文件或招标文件拆成可核对的资格、材料、报价、签章、时间和提交 Checklist，并保留原文定位。用于投标准备、材料收集和提交前复核。
---

# 标书拆解

## 触发条件
- 收到招标/采购文件
- 需要快速找必交材料
- 投标文件准备或提交前复核
- 采购文件出现补遗或变更

## 输入
- 采购/招标文件及附件
- 补遗、答疑或更新版本
- 公司已有资质清单（如有）
- 计划提交方式与截止时间

## 执行步骤
1. 提取截止时间、提交方式、资格条件、授权、资质、业绩、报价表、保证金、签字盖章、装订和格式要求。
2. 每项记录原文定位、责任人建议、状态和风险级别。
3. 识别否决项、互相冲突要求和需人工核实项。
4. 合并补遗变更并标记被替换要求。
5. 生成执行 Checklist 与提交前复核表。

## 人工确认边界
- “已满足/已提交”状态只能由人工或可靠业务记录确认。
- 最终投标完整性必须人工复核，AI 不承诺无遗漏。

## 输出 Artifact
- `business.tender.checklist`
- `business.tender.risk-items`
- `business.tender.final-review`

## 禁止
- 不得臆造资质、业绩、授权、盖章状态或提交成功。
- 没有原文依据时不得把项目标为强制项。

## 所需工具与权限

工具：
- `business.document.extract`
- `business.source.locate`
- `business.artifact.create`
- `business.approval.request`

权限：
- `document.read:tender`
- `artifact.write:draft`
- `project.read`
