# 第 20–21 天：数据迁移、升级回滚本机 QA

## 门禁结论

- 隔离数据迁移/升级覆盖/失败回滚/卸载保留：**本机自测通过**。
- PowerShell 语法解析：**通过**。
- 安全 dry-run：**通过**，默认只规划 `.runtime` 下的隔离路径，不访问真实用户 AppData。
- 真实 NSIS 双安装包升级：**待候选包实跑**；当前没有把这一步伪装成已通过。
- 未修改业务代码、数据库实现、任务引擎、文档引擎或执行报告。

## 写入范围

本次只涉及：

- `scripts/invoke-nsis-release-acceptance.ps1`
- `scripts/invoke-data-migration-rollback-acceptance.ps1`
- 本 QA 文件

`invoke-nsis-release-acceptance.ps1` 仍是实际 NSIS 发布验收入口；新增脚本是本机可重复的隔离回滚自测，不替代真实安装包验收。

## 覆盖内容

实际 NSIS 验收入口新增以下隔离数据保护范围：

1. SQLite Ledger：`ledger/.nsis-acceptance-sqlite-sentinel.json`
2. Local Vault：`vault/.nsis-acceptance-vault-sentinel.json`
3. credentials：`credentials/.nsis-acceptance-credentials-sentinel.json`
4. brain workspace：`codex-home/workspaces/.nsis-acceptance-brain-workspace-sentinel.json`
5. Business Workbench workspace：`vault/.business-workspace-staging/.nsis-acceptance-business-workspace-sentinel.json`

升级前会在当前 `RunId` 的隔离目录下生成全量 SHA-256 快照和备份。若升级后流程失败，脚本会先停止测试进程，把失败数据树移动到 `data-rollback/failed-state`，再恢复 `data-rollback/backup`，并重新比对快照。失败状态不直接删除，便于复盘。

卸载门禁只允许清理隔离 `install` 目录和测试注册表项；数据根必须在卸载后仍保留。所有生成路径都必须位于 `.runtime` 下，且路径链上的 reparse point 会被拒绝。

## 已执行验证

```powershell
# 现有 NSIS 入口：只做计划，不安装、不启动、不改注册表
./scripts/invoke-nsis-release-acceptance.ps1 `
  -DryRun `
  -RunId migration-gate-dryrun `
  -Version 1.3.4 `
  -InstallerPath ./src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe

# 新增隔离回滚脚本：只输出 JSON 计划，不落真实 AppData
./scripts/invoke-data-migration-rollback-acceptance.ps1 `
  -DryRun `
  -RunId migration-gate-dryrun

# 新增隔离回滚脚本：执行完整本机自测并保留证据
./scripts/invoke-data-migration-rollback-acceptance.ps1 `
  -RunId migration-gate-selftest `
  -KeepArtifacts
```

本机结果：

- 两个脚本均通过 PowerShell AST 语法解析。
- NSIS 入口 dry-run 正常结束，并明确提示未提供不同 `PreviousInstallerPath`，因此不宣称跨版本升级通过。
- 隔离回滚 dry-run 输出 `status=planned` JSON。
- 隔离完整自测输出 `status=passed` JSON，覆盖 `fixture`、`upgrade`、`backup`、`rollback`、`uninstall` 五步。
- 自测证据目录：`.runtime/data-migration-rollback/migration-gate-selftest/`。
- `git diff --check` 无新增空白错误；已有工作树仅出现换行风格提示。

## 真实候选包门禁

候选版必须提供两个不同文件名和版本的 NSIS 安装包，然后在干净 Windows 测试账户或 VM 执行：

```powershell
./scripts/invoke-nsis-release-acceptance.ps1 `
  -RunId release-<previous>-to-<candidate>-migration-rollback `
  -Version <candidate-version> `
  -PreviousInstallerPath <previous-installer.exe> `
  -InstallerPath <candidate-installer.exe> `
  -ColdStartCount 20
```

若要验证失败恢复路径，在隔离账户执行同一命令追加 `-InjectFailureAfterUpgrade`。该参数只在当前隔离验收流程中注入失败，不应在正常候选版通过命令中使用。预期结果是：命令失败并生成 `acceptance-summary.json`，其中 `dataPreservation.rollbackCompleted=true`，失败数据保留在 `data-rollback/failed-state`，隔离 AppData 哨兵全部可恢复。

## 当前阻塞

唯一未在本机本轮完成的是**真实 NSIS 双版本安装包的跨版本升级、实际应用启动和真实卸载器回滚**。原因不是脚本阻塞，而是该门禁必须使用明确的 previous/candidate 安装包并在隔离 Windows 账户或 VM 执行；不能用同一个安装包或合成 fixture 冒充。
