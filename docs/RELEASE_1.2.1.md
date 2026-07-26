# 半山商务工作台 1.2.1 发布说明

- 产品：半山商务工作台
- 版本：`1.2.1`
- 发布日期：`2026-07-23`
- 当前状态：**内部验收通过，可发布内测包**
- releaseStatus: final-internal-accepted

## 本次修复

### AI 服务设置

- 修复新增 OpenAI-compatible 服务后无法保存、保存无反馈和默认服务不可切换的问题。
- “检查配置”调整为“拉取模型”，直接请求 Provider 的 `GET /models`，使用 Bearer API Key，并读取标准 `data[].id` 模型列表。
- 新增服务可在保存前拉取模型；拉取成功后自动填入去重、排序后的模型列表，并保留或选择有效默认模型。
- 保存、拉取模型和设为默认均提供明确成功状态；缺少模型、API Key 或默认模型时给出可执行的中文提示。
- 已保存服务拉取模型时由 Rust Host 读取 DPAPI 凭据，前端无需回传明文密钥；新建服务拉取不会提前持久化 API Key。
- 网络层禁止重定向，连接超时 5 秒、请求超时 15 秒，响应上限 1 MiB，模型上限 128，模型 ID 上限 256 字节。
- 401、403、404、429、超时、网络失败、非法 JSON、空模型列表和超大响应均返回明确错误，日志、Debug、序列化和持久状态不包含 API Key。

### 界面与可读性

- 全局改为 Codex Desktop 风格的苹果白、系统浅灰、黑灰文字和克制蓝色交互色，移除可见粉色主题。
- 设置中心、主工作台、对话、合同审查、需求、报价单据和归档统一提升字号；主要正文和控件为 13–14px，辅助信息不低于 12px。
- 设置中心的桌面与窄窗口布局完成实测，390px 宽度下保持单列编辑、横向分类导航和无页面级横向溢出。
- 输入框、按钮、状态、焦点和错误反馈采用一致的中性灰与蓝色层级。

## API 与人工冒烟

- 真实内部 Provider `/models`：HTTP `200`，返回 `19` 个模型，包含 `gpt-5.6-sol`。
- 模拟人工路径：新增服务、填写 URL/Key、拉取模型、默认模型选择、保存、再新增第二服务、切换默认、错误 Key 和错误 URL，全部通过。
- 桌面主界面与设置中心截图复核：无可见粉色、无文本重叠、无横向溢出。

## 安装包

- 文件名：`半山商务工作台_1.2.1_x64-setup.exe`
- 文件大小：`131276397` bytes
- SHA-256：`0c600b922465e7eff650966b773731955e265370dbf92fa052201b92e7a69404`
- Authenticode：**NotSigned（未签名）**

产品安装包尚未配置公司代码签名证书，Windows 可能显示未知发布者或 SmartScreen 提示。官方 Codex sidecar 自身签名状态按最终构建校验结果为准。

## 质量门

- 协议生成：`254 passed / 0 failed`
- 前端：`21 files / 125 tests passed`
- Rust：`579 passed / 0 failed / 4 ignored`，Business Tool Registry `24 passed`
- TypeScript、Vite build、Rust fmt/check/clippy：`passed`
- 官方 Codex app-server 真实握手：`1 passed / 0 failed`
- Codex sidecar：`codex-cli 0.144.5`，Entrypoint `efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b700e22a`，`341195568` bytes，Manifest `7442f28332324cc613317fa3bd315ebc8c1d967ff1b131144a70ddb337030abe`，LICENSE/NOTICE/Authenticode `passed`（官方 sidecar 签名有效）
- `git diff --check`：`passed`（仅既有换行符提示）

## 跨版本验收

- RunId：`release-1.2.1-20260723-final-r2`
- 升级类型：`cross-version-upgrade`
- 初始版本：`1.2.0`
- 最终版本：`1.2.1`
- 旧包 SHA-256：`cc06d501ac5a1e67dbfb103c2701d037b487c74c45bd96b1bc8d8dd26338b0e5`
- 最终包 SHA-256：`0c600b922465e7eff650966b773731955e265370dbf92fa052201b92e7a69404`
- 结果：`passed`（初始安装、首启、覆盖升级、双 PID 重启、静默卸载、数据保留和注册表恢复全部通过）

验收覆盖 1.2.0 隔离安装与首启、SQLite/Vault 建立、1.2.1 覆盖升级、升级前后权威数据快照一致、内测 Provider DPAPI 状态验证、Codex LICENSE/NOTICE 校验、两次独立 PID 重启、静默卸载、测试注册表清理和既有注册表快照恢复。

## 内测边界

- 本安装包包含内测 API 凭据，只允许公司内部受控分发；不得公开上传或对外发送。
- R2、飞书 CLI 和在线更新仍以本地优先与显式状态为边界，不得把未配置通道显示为已连接。
- 网页版未实现；`WebHostAdapter` 仅用于冻结协议和测试可迁移边界。
- 正式外发前必须配置产品代码签名、轮换内测凭据，并接入可撤销的凭据分发机制。
