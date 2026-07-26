---
name: contract-version-compare
description: 对比合同初稿、客户修订稿和签署稿，定位新增、删除、金额日期变化及风险变化，输出逐条版本差异和待确认事项。
---

# 合同版本对比

## 触发条件
- 客户回传合同修订版
- 签署前需要核对最终版与已审版本
- 需要解释新增、删除和替换了哪些条款

## 输入
- 基准合同版本
- 待比较合同版本
- 可选的已确认 Brief、报价和历史审查报告

## 执行步骤
1. 确认两份文档的身份、版本和完整性，禁止用错基准。
2. 按条款和表格语义对齐，区分新增、删除、修改、移动和格式变化。
3. 重点比较主体、金额、税率、付款、交付、验收、知识产权、保密、违约、终止和争议解决。
4. 对每项实质变化保留双边原文定位，说明影响和风险级别变化。
5. 与最近一次人工确认结论核对，生成签署前最终检查清单。

## 人工确认边界
- 合同版本身份、基准选择和所有实质条款变化必须由授权人员确认。
- AI 只做业务差异识别，不替代法律意见。

## 输出 Artifact
- `business.contract.version-diff`
- `business.contract.changed-risks`
- `business.contract.final-checklist`

## 禁止
- 不把纯格式变化误报为实质变化。
- 不遗漏附件、补充协议、报价表或盖章页中的变化。

## 所需工具与权限

工具：
- `business.document.extract`
- `business.source.locate`
- `business.artifact.compare`
- `business.project.read`
- `business.artifact.create`
- `business.approval.request`

权限：
- `document.read:contract`
- `project.read`
- `artifact.read`
- `artifact.write:draft`
