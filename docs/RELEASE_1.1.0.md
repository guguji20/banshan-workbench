# 半山商务工作台 1.1.0 发布说明

- 产品：半山商务工作台
- 版本：`1.1.0`
- 发布日期：`2026-07-22`
- 当前状态：**正式内测发布，最终安装包验收通过**
- releaseStatus: final-internal-accepted

## 本次交付

### 商务闭环

- 客户/项目资料、确认需求采用、报价、合同、请款、验收、到账、回款台账与归档统一在本地工作区维护。
- 当前正式报价必须先登记客户确认凭证，才能进入合同流程。
- 合同和验收单确认生效时必须选择签署/验收凭证，或明确填写人工豁免原因。
- 单据作废必须填写原因，原始记录与审计事件永久保留。
- 到账改为不可变流水账，支持分次到账、凭证、部分到账、冲销和重复流水号拦截。
- 合同审查通过后可选择签署合同文件并纳入正式合同链路。
- 新增相关 UI 流程守卫与回归测试，避免后端能力存在但桌面操作不可达。

### AI 与设置

- 设置中心支持新增、编辑、删除和切换 OpenAI-compatible Provider。
- 每个 Provider 可维护模型列表并选择默认模型，Agent 输入区可按 Thread/Turn 切换模型。
- 内测默认 Provider 为半山 AIGC，默认模型为 `gpt-5.6-sol`。
- 内测凭据随本安装包预置，首次启动后写入当前 Windows 用户的 DPAPI 保护区；同事无需手工填写。
- 至少保留一个 AI Provider，避免删除最后一个 Provider 后重启出现隐式恢复。
- 设置中心提供 SQLite、Local Vault、缓存、暂存区和凭据区状态，支持打开目录和安全清理可再生缓存。
- Cloudflare R2 与飞书 CLI 当前为渠道壳；R2 始终只作为异步备份，本地完成状态不等待线上备份。

## 本地权威

- SQLite Task Ledger 是命令、任务、事件和商务记录权威。
- Local Vault 是合同原件、凭证、报告和生成单据权威。
- R2 未配置、断网或上传失败不会阻断本地商务流程。
- React 只通过 Client SDK 使用 typed Tauri IPC，不直接持有 Provider、R2 或 Codex 凭据。
- 官方 Codex app-server `0.144.5` 是唯一 Agent Runtime。

## 安装包

- 文件名：`半山商务工作台_1.1.0_x64-setup.exe`
- 文件大小：`130990814` bytes
- SHA-256：`759eb2b69bf736d4199e08c0c9ca6aec547c3a12a1207b2dc806cfeacab3dfda`
- Authenticode：**NotSigned（未签名）**

产品安装包尚未配置公司代码签名证书，Windows 可能显示未知发布者或 SmartScreen 提示。官方 Codex sidecar 自身的 Authenticode 签名为 `Valid`，签名主体为 OpenAI OpCo, LLC。

## 质量门

- 协议生成：`224 passed`
- 前端：`20 files / 119 tests passed`
- Rust：`525 passed / 0 failed / 4 ignored`
- Business Tool Registry：`24 passed`
- TypeScript、Vite build、Rust fmt/check/clippy：全部通过
- Codex sidecar Manifest、SHA-256、LICENSE、NOTICE、Authenticode：全部通过
- `git diff --check`：通过

## 跨版本验收

- RunId：`release-1.1.0-20260722-final`
- 升级类型：`cross-version-upgrade`
- 初始版本：`1.0.0`
- 最终版本：`1.1.0`
- 旧包 SHA-256：`0be38deebc31c3b32a0f2d7807ae3d1a098fa7c4d80f27b9371c58ae5cf3911e`
- 结果：`passed`

验收覆盖旧包安装与首次启动、SQLite/Vault 建立、最终包覆盖升级、数据快照一致、内测 Provider DPAPI 状态解密、endpoint 与 `gpt-5.6-sol` 精确匹配、Codex 法律文件、两次独立 PID 重启、静默卸载、测试注册表清理和既有注册表恢复。

## 内测边界

- 本安装包包含内测 API 凭据，只允许公司内部受控分发；不得公开上传或对外发送。
- R2 设置、飞书 CLI 和在线更新在本版本只保留接口与状态壳，不承诺完整连接能力。
- 网页版未实现；`WebHostAdapter` 仅用于冻结可迁移协议。
- 正式外发前必须更换产品签名、轮换内测凭据并接入可撤销的凭据分发机制。
