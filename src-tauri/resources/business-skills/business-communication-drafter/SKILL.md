---
name: business-communication-drafter
description: 把模糊需求、合同意见、报价差异、请款验收问题和回款事项整理成可直接确认的追问、回复或内部交接草稿。
---

# 商务沟通与追问

## 触发条件
- 客户反馈模糊、摇摆或前后矛盾
- 需求、报价、合同、请款、验收或回款存在缺失信息
- 需要把内部专业判断转成客户可理解的回复
- 需要形成清楚的内部交接

## 输入
- 当前项目主档和已确认事实
- 相关原文、反馈、版本差异或异常记录
- 沟通对象、目标和期望时限

## 执行步骤
1. 区分已确认事实、推测、冲突和缺失，先删除重复问题。
2. 只提出会影响下一步的关键问题，优先给出可选项和判断维度。
3. 对合同、报价、请款、验收和回款事项引用对应 Artifact 或原文定位。
4. 生成客户版和内部版两种表达：客户版简洁友好，内部版保留风险和责任人。
5. 标记必须人工确认的承诺、价格、日期、责任、法律和付款内容。

## 人工确认边界
- 所有对外发送、价格承诺、交付承诺、合同立场和回款安排必须人工确认。
- 不代表客户或公司作出未确认承诺。

## 输出 Artifact
- `business.communication.question-list`
- `business.communication.reply-draft`
- `business.communication.internal-handoff`

## 禁止
- 不捏造客户态度、公司政策或历史承诺。
- 不把内部风险判断原样外发。

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
- `artifact.read`
- `artifact.write:draft`
