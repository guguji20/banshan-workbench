---
name: business-case-orchestrator
description: 识别商务案件阶段，把复杂事项拆成按依赖排序的短任务、检查点、负责人、输入输出、审批节点和恢复点，复用统一 Task、Artifact 与 Ledger。
---

# 商务案件编排

## 触发条件
- 一个商务事项横跨需求、报价、合同、请款、验收或回款
- 需要明确阶段、依赖、负责人、截止时间、审批和恢复位置
- 案件被临时打断，或需要告诉用户“现在卡在哪里、下一步做什么”

## 输入
- 客户和项目目标
- 当前业务阶段、截止时间和责任角色
- 相关主档、Artifact、Ledger 状态和已知限制

## 执行步骤
1. 识别当前阶段和完成标准，区分已完成、进行中、阻断、待审批和不在范围。
2. 按真实依赖拆成短任务，每项声明输入、动作、输出、责任角色、截止时间和成功判定。
3. 标记人工审批、外部材料、跨阶段一致性检查和不可逆操作。
4. 将计划映射到现有 Task、Artifact、项目阶段和 Ledger，不创建平行任务系统。
5. 分批推进；每批结束更新检查点、结果、阻断项和下一步，不盲猜越过缺失材料。
6. 恢复时从最近检查点和 Artifact revision 继续，不重新生成已经确认的事实。

## 人工确认边界
- 最终报价、合同立场、付款、验收、对外承诺和项目关键日期必须审批。
- Agent 可以拆解、排序和建议，不替代负责人接受风险。

## 输出 Artifact
- `business.case.execution-plan`
- `business.case.stage-status`
- `business.case.blockers`
- `business.case.handoff`

## 禁止
- 不创建第二套任务引擎、审批引擎、业务数据库或 Agent Runtime。
- 不为了形式把简单事项拆成冗长计划。

## 所需工具与权限

工具：
- `business.project.read`
- `business.artifact.read`
- `business.ledger.read`
- `business.artifact.create`
- `business.approval.request`
- `business.task.plan`

权限：
- `project.read`
- `artifact.read`
- `artifact.write:draft`
- `ledger.read`
- `task.submit`
