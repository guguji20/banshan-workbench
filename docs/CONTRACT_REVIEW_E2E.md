# 合同审查最小闭环验收

## 目的

`scripts/verify-contract-review-e2e.ps1` 验证当前桌面版合同审查的本地最小闭环，不要求真实 Provider 或 R2 可用，也不会运行 Anybox/Proma。

覆盖链路：

```text
导入 DOCX 合同
-> Local Vault 原件与 SHA-256
-> 文档提取
-> 本地规则审查
-> Agent 缺凭据时降级并保留规则结果
-> 人工逐条确认 Finding
-> HTML/DOCX 审查报告 Artifact
-> 报告写入 Local Vault
-> SQLite 保存完整审查图和命令回执
-> R2/备份队列失败不影响本地 Completed
```

## 为什么不直接驱动安装包

安装包中的合同能力通过以下入口工作：

```text
React -> BsaigcClient -> DesktopHostAdapter -> Tauri IPC
      -> execute_contract_review_command -> Rust service/runtime
```

当前安装后的 EXE 没有公开合同审查 CLI，也没有测试专用 HTTP 端口。为避免增加生产后门或依赖脆弱的坐标点击，本验收静态检查 Tauri command、Client SDK 映射和 NSIS 配置，然后通过 Rust 生产服务层的确定性测试执行同一业务实现。

Cargo 调用使用 `--lib`，只编译和执行合同审查、Vault、SQLite 与 R2 生产模块内的定向用例，不受其他并行开发中的集成测试影响。

脚本会记录当前 `release/<version>/*setup.exe` 的存在状态与 SHA-256（如安装包存在），但不会安装、卸载或启动它。

## 运行

在仓库根目录执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-contract-review-e2e.ps1
```

默认证据目录固定为：

```text
.runtime/contract-review-e2e/latest/
```

重复执行会覆盖同名摘要和日志，QA 夹具由现有生成器确定性重建，因此不会累积业务数据。需要保留单独一次记录时：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-contract-review-e2e.ps1 -RunId 20260722-nightly
```

任一步失败时脚本返回非 0，并尽量写出失败摘要。

## 验收项

| 步骤 | 真实验证内容 |
|---|---|
| `static-interface` | Tauri command、invoke 注册、DesktopHostAdapter 映射、NSIS 配置和安装包元数据 |
| `fixture-generation` | 使用现有标准库脚本生成确定性中文 DOCX 合同 |
| `fixture-integrity` | 合同字节数与 SHA-256 和 manifest 一致 |
| `closed-loop` | 导入、Vault、提取、规则、Agent 降级、人工决策、双格式报告、Artifact、备份入队 |
| `restart-persistence` | 关闭并重新打开 SQLite 后，完整审查图仍可读取 |
| `command-idempotency` | 同一幂等命令重放不重复改状态或追加事件 |
| `backup-outbox-nonblocking` | 备份队列写入失败时，本地审查仍为 `Completed`，Artifact 仍可读 |
| `r2-network-nonblocking` | R2 网络失败只使备份失败，不删除或降级 Local Vault 与本地资产记录 |

## 证据

```text
.runtime/contract-review-e2e/<RunId>/
|- preflight.json
|- summary.json
`- logs/
   |- closed-loop.stdout.log
   |- closed-loop.stderr.log
   `- ...
```

`summary.json` 是机器可读的权威结果。只有 `status` 为 `passed` 且进程退出码为 `0` 才算通过。

证据不记录 API Key、R2 密钥或 Provider 响应。所选测试使用 `MissingCredentialAgent` 和 fake R2 transport，不发起真实 Provider/R2 网络请求。

## 当前边界

本套验收不覆盖：

- 真实 Codex Provider 的合同智能审查质量。
- 真实 R2 上传、下载与云端一致性。
- 扫描 PDF 的 Windows OCR 路径。
- WebView 视觉、键鼠操作和可访问性。
- 安装、覆盖升级、卸载；这些由 `scripts/invoke-nsis-release-acceptance.ps1` 独立验收。

这些边界不影响本脚本对“本地闭环在 Agent/R2 不可用时仍可完成”的判定。
