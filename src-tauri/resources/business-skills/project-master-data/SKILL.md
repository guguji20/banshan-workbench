---
name: project-master-data
description: 从已确认 Brief、报价和合同中抽取并维护唯一项目主档，供请款、验收和一致性审计复用。用于项目立项、合同确认后建档或主数据变更。
---

# 项目主数据抽取

## 触发条件
- 项目首次建档
- 合同或报价确认后同步主数据
- 客户、金额、交付或付款节点变化
- 下游材料需要统一数据源

## 输入
- 已确认 Brief
- 经审批报价
- 合同审查结果与合同版本
- 现有项目主档及 revision（如有）

## 执行步骤
1. 按共享 schema 提取客户、项目、合同、金额、税率、交付、付款、验收、联系人和关键日期。
2. 标记每个字段的来源 Artifact、版本和定位。
3. 与现有主档对比，生成字段级变更集和冲突项。
4. 先生成写入预览；人工确认后以新 revision 保存，不覆盖历史。

## 人工确认边界
- 新增或修改客户主体、金额、税率、付款、交付和验收字段必须人工确认。
- 没有当前 revision 时禁止盲写；冲突未解决时只生成草稿。

## 输出 Artifact
- `business.project-master.preview`
- `business.project-master.change-set`
- `business.project-master.revision`

## 禁止
- 不得用“常见做法”填充未知字段。
- 不得静默覆盖已有主档或删除历史 revision。

## 所需工具与权限

工具：
- `business.artifact.read`
- `business.source.locate`
- `business.project.read`
- `business.project.write`
- `business.approval.request`

权限：
- `project.read`
- `project.write:approved`
- `artifact.read`
- `audit.write`
