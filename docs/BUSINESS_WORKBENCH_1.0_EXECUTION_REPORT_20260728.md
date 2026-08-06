# 半山商务工作台业务版 1.0 执行报告

> 记录日期：2026-07-28  
> 最新事实复核：2026-08-05  
> 当前分支 / HEAD：`main` / `206bc83`  
> 状态：主计划第 1-19 天功能纵切和本机 Windows 构建、真实跨版本升级、隔离回滚已闭环，当前进入统一正式发布门禁收口；第二台 Windows/独立用户、macOS 构建签名公证与启动、Windows Authenticode、干净 Git 发布基线及第 22-28 天五个真实商务使用全部完成前，不发布任何内部 RC、平台 RC 或跨平台 RC

## 1. 唯一执行合同

- 唯一产品与验收合同：`docs/BUSINESS_WORKBENCH_1.0_MASTER_PLAN_20260728.md`。
- 本次复核按 UTF-8 读取；文件 SHA-256：`2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`。
- 执行策略固定为“新业务壳 + 复用成熟底层”：旧业务界面、旧版路由和旧壳文件已下线，不重写 SQLite、Local Vault、Auth、OCR、R2、Task/Event、合同审查、文档生成及桌面发布底座；历史数据继续通过迁移、SDK 和 1.0 历史资料入口兼容读取。
- 主计划产品范围和验收标准保持不变；顶部执行状态已校准为“开发进行中”。

## 2. 当前工作树

- 当前 `git status` 共 `226` 项：工作树仍包含新 1.0 产品壳与业务能力、旧 UI 物理删除、Client SDK/HostAdapter、Rust 服务与协议、发布脚本、测试和验收文档，均未提交；本轮未新增产品源码变更，仅追加本地验收证据与本报告记录。
- 变更已覆盖主计划第 1-21 天的大部分实现与本机门禁，但远端 `origin/main` 仍停留在删除旧 UI 之前的 `206bc83`，不能从远端旧提交生成当前候选版。
- 本报告只记录当前事实，不将未提交代码视为发布基线。

## 3. 已完成实现

### 3.1 新壳与路由

- 新增 Codex 式 1.0 默认入口和响应式三栏工作台；当时曾临时通过 `?legacy=1` 保留旧 `App.tsx`，该过渡入口已于 2026-08-02 在第 25 节完成物理删除。
- 已建立报价、合同审查、验收、结算、归档和资料检索的独立任务路由；报价与验收不互相构成前置条件。
- 已接入文件/文件夹添加、拖拽文件、粘贴图片、项目与对话隔离，以及无项目时禁止发送等基本约束。
- 新建项目已改为应用内表单，不再依赖系统 `prompt()`；项目名称必填，客户名称可补充，提交期间会锁定重复操作。

### 3.2 工作区持久化

- Rust 侧新增 SQLite 表 `brain_project_workspaces`，保存项目与本地文件夹绑定关系。
- 绝对路径仅保留在 Rust 侧；前端协议只接收稳定标识和临时访问令牌。
- 已实现启动后令牌重发、绑定/解绑 revision CAS 以及对应 SDK、Desktop HostAdapter 和协议类型接线。

### 3.3 真实业务数据接线

- 新界面已读取现有持久化 `BusinessWorkspaceRecord`，展示真实任务卡、缺失资料、预览、版本和审批上下文。
- 审批流复用现有 `changeBusinessDocumentStatus`；批准后的文件生成复用 `generateBusinessDocument`，报价走 XLSX，其他业务文档走 DOCX。
- 新业务界面没有另建第二套任务、审批或业务数据库。

### 3.4 验收缺失门禁纵切

- 在现有业务 SQLite 中增加验收批次和验收素材持久化，批次可配置 6 个内容槽位和 5 个输出规格；未增加第二套数据库、任务引擎或文档状态机。
- 新增 `businessWorkspace.createAcceptanceBatch` 和 `businessWorkspace.upsertAcceptanceMaterial`，复用现有命令回执、revision CAS、领域事件和重启恢复。
- 验收素材必须引用同项目 Ready Asset；readiness 只统计已确认、非重复、`groupKey` 去重后的素材组，白鹅潭 `required=4 / provided=3 / missing=1` 已稳定回归。
- 未就绪批次允许创建和提交草稿复核，但禁止批准和正式生成；补齐后放行，批准后素材失效会再次阻止生成。
- 验收文档快照冻结 `acceptanceBatchId`；旧验收文档没有批次关联时保留兼容行为。批次状态由关联的现有业务文档推导。
- 新界面右侧缺失面板已展示真实 `4/3/1` blocker；复核批准和生成按钮按 readiness 禁用，草稿提交复核不被误阻断。

### 3.5 验收 5 草稿准备纵切

- 验收输出规格已增加稳定 `outputCode`、唯一 `format` 和 `requirementIds`；文档快照冻结 `acceptanceOutputSpecId`、批次 revision 和素材绑定 manifest。
- 新增 `businessWorkspace.prepareAcceptanceDocuments` 及 `businessWorkspace.acceptanceDocumentsPrepared`，在一个 SQLite 事务内只创建当前批次缺失的草稿；同批次同输出规格有唯一索引，重复命令不会产生重复文档。
- 草稿和复核中文档会随素材变化刷新冻结快照；已批准文档保持不可变。输出格式必须与输出规格一致。
- 新界面已接入批次 `0/5`、部分完成和 `5/5` 统计、准备按钮与瞬时忙碌状态；材料不齐时允许先准备草稿，但审批面板和任务卡继续同时阻止批准及生成。
- 已注册白鹅潭 5 个稳定真实模板键并锁定 `4 DOCX + 1 XLSX`；内置验收模板继续兼容旧数据。当前注册只建立身份和格式边界，不代表真实模板克隆渲染完成。

### 3.6 真实模板证据基线

- 已只读核对 `真实需求/瑞玺AI请款资料/【空白验收模版】` 下 5 个物理文件和 6 类业务内容：视频成片验收、制作成果确认、服务结算清单、合同结算 XLSX，以及同一 legacy DOC 承载的付款申请与合同结算计算。
- DOC/DOCX 映射见 `docs/ACCEPTANCE_DOC_TEMPLATE_MAP_20260729.md`；共核对 5 份 Word 模板、5 份历史 PDF、42 页，模板 SHA-256 已复算。
- XLSX 映射见 `docs/ACCEPTANCE_XLSX_TEMPLATE_MAP_20260729.md`；正式输出页为 `附件1最终结算书`，必须克隆模板并保留 5 个公式、3 个合并区、打印区、分页和 4 条签章横线，同时移除 3 个红色“盖公章”提示椭圆。
- 真实素材只有 3 组，每组包含脚本 DOCX 和 3 张截图；未发现第 4 组、MP4、花絮或发布数据，进一步确认实际基线就是 `required=4 / provided=3 / missing=1`。
- legacy 付款申请模板残留真实账户和旧金额，正式自动生成必须执行旧值清零断言；历史 PDF 没有实际签章，只能作为版式基线。

### 3.7 真实模板资产冻结与适配器安全门禁

- 验收输出规格和文档快照已冻结 `templateAssetId`、`templateSourceSha256`、`templateMappingVersion`；旧 JSON 缺少字段时默认 `null/null/""`，协议向后兼容。
- 创建验收批次时，真实模板键必须完整绑定三项来源信息；内置模板禁止绑定外部来源。模板 Asset 必须为同项目 Ready Document，Vault 完整性、SHA-256 和 `.docx/.xlsx` 格式全部匹配；legacy `.doc` 被明确阻断，等待规范化为受管 DOCX。
- 合同结算 XLSX 适配器已改为单次受限读取，消除 SHA 校验 TOCTOU；增加 ZIP 条目数、单条大小、总解压量和压缩比上限，清理未引用 shared strings，强制 `H16` 空白，且跨文件系统发布不得覆盖已有目标。
- 服务结算 DOCX 适配器当前只允许已验证的 1-3 行；保留两段复选框和符号字体，表头重复、表头与数据行禁止跨页拆分，克隆行清除重复 OOXML 段落 ID，并阻断 embeddings、OLE/package、`w:object` 和 DDE 活动内容。
- 两个真实模板只读回归已显式执行通过。合同结算和服务结算键已接入专用渲染器；其余 3 个尚未实现的真实模板键继续返回 `BUSINESS_TEMPLATE_RENDERING_NOT_READY`，绝不回落到通用 DOCX/XLSX 生成器产出伪正式件。

### 3.8 合同结算与服务结算正式生成链

- 验收输出规格和不可变文档快照已增加合同结算金额字段及最多 3 行服务结算字段；合同总额使用整数分和 `checked_add` 校验，最终金额必须为整元，质保比例限制在 `0..=10000 bps`。
- 服务结算草稿允许 `providedAsRequired=null`，送审、批准和生成时必须显式确认；未按要求提供时必须填写备注。合同结算和服务结算模板分别锁定专属 `outputCode`、格式和精确 `templateMappingVersion`。
- 正式生成从后端 Vault 使用同一文件句柄限量读取模板字节，并同时完成 Ready、项目、Document 类型、扩展名、数据库 SHA、快照 SHA 和注册 SHA 校验；模板路径不进入文档引擎、协议、事件或回执。
- DOCX 与 XLSX 适配器均消费同一份已验签字节完成 ZIP 解析，消除“先算 SHA、再重新打开路径”的 TOCTOU；适配器成功后才进入生成 Asset 导入，原有 receipt-first、revision CAS、竞争赢家和孤儿 Asset 清理语义保持不变。
- 已新增真实业务命令链回归：真实模板先导入现有 Vault，再创建验收批次、冻结快照、准备草稿、送审、批准、生成并验证 Ready 输出 Asset；合同结算 XLSX 与服务结算 DOCX 均通过。
- 协议生成脚本已固化 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`，并把两类新增 TypeScript 绑定列入必检清单，避免 8 GB Windows 环境重复高内存链接。

### 3.9 Legacy 付款模板规范化与专用生成链

- legacy `.doc` 仅通过 Microsoft Word COM 在私有临时目录规范化为宏禁用 DOCX；原文件只读，转换有 120 秒超时、Word 进程归属清理、SHA-256、ZIP/XML、宏/OLE/ActiveX、外链和大小审计。
- 新增受管模板版本和 `PendingReview -> Approved/Rejected` 单向审批；规范化 Asset 复用现有 Vault、事件、回执、revision CAS、孤儿 GC、共享 staging lease 和重启恢复。
- 付款申请与合同结算计算共用同一真实模板，但由一个专用 renderer 同时生成两页内容；动态结算行、累计已付、剩余应付、整分和大写金额均使用冻结数据确定性计算，不回落到通用伪正式生成器。
- 正式生成要求 Approved normalizedTemplate，生成前再次核对模板 Asset、mapping version、付款计划、回款、发票台账和银行账户稳定指纹；部分红冲及全额红冲归零都会阻断旧快照生成。
- renderer 对 DOCX 全包的 XML、RELS 和常见文本部件执行旧示例值、未解析占位符和未知长账号检查，覆盖 `customXml` 和自定义属性；宏、OLE、外链及畸形 XML 继续 fail-closed。
- 模板审批不再只检查文件完整性：付款模板进入 Approved 前会读取同一已验签字节并执行标题、段落、分页、账户表和 11 行/12 逻辑列结算表结构预检。
- 当前残余业务风险：合同标题、编号和金额尚未绑定有效合同 ID/revision；联行号仍来自付款申请冻结输入，等待公司银行账户独立版本模型。

## 4. 已完成验证

- 前端测试：`28/28` 个测试文件、`172/172` 项测试通过；TypeScript 全量检查通过。
- 前端生产构建通过；新版默认 JS 约 `337.43 kB`（gzip `96.13 kB`），旧版延迟 JS 约 `340.32 kB`（gzip `91.98 kB`）。
- 上一稳定轮 Rust 全量门禁为 format/check/clippy、library 和 integration 全绿；本轮付款与认证增量完成后必须重新执行全量门禁，不沿用旧结论冒充当前工作树结果。
- 协议导出专项最近一次 `299/299` 通过；模板审批、付款申请数据和新增 TypeScript 绑定均已列入生成脚本必检清单。
- 模板单次 Vault 读取回归 `3/3`、两个字节渲染入口回归 `2/2`、业务工作区模块 `46/46` 通过；两个适配器普通测试合计 `31 passed`。
- 付款专项 `13/13` 通过，包含隐藏 `customXml`/自定义属性、畸形 XML、全额红冲归零和真实 legacy DOC 规范化渲染；账户与发票漂移、审批、GC 和共享 staging 专项另行通过。
- 4 个 ignored 真实模板测试已用只读原件显式执行通过：合同结算 XLSX、服务结算 DOCX、两者完整业务命令链，以及付款 legacy DOC 规范化与专用渲染。
- 工作区持久化聚焦测试 `5/5` 通过：重启重新签发临时令牌、绝对路径不进入 DTO、revision CAS 冲突和解绑后重启均已覆盖。
- 验收领域测试 `7/7`、持久化聚焦测试 `5/5`、前端验收投影测试 `9/9` 通过；覆盖跨项目 Asset 拒绝、`4/3/1`、补齐放行、批准后素材失效、重启恢复和旧工作区兼容。
- 浏览器烟测通过：新版默认入口可加载，`1440 × 900` 和 `390 × 844` 下页面宽高与视口一致且无横向溢出；移动端双侧栏默认收起；新建项目表单可打开、输入和正确切换提交按钮状态。
- Web 预览仍使用未实现持久化命令的 `WebHostAdapter`，因此不能替代 Tauri 桌面端到端验证；该限制已单独记录，不视为项目创建成功。
- `git diff --check`、业务技能包 `14 skills / 14 tools / 35 files` 校验通过；意外产生的第二套未接线文档引擎已删除。

## 5. P0：报价税额与优惠口径（已修复）

- 已修复旧持久化报价链路的重复计税：业务服务计算含税行金额、项目优惠和冻结总额，`document_engine.rs` 只渲染冻结结果。
- 在白鹅潭基准 `21,200 × 4`、税率 `6%` 下，该链路可能先得到 `89,888`，再使 XLSX 结果进一步偏离至约 `95,281.28`。
- `BusinessProfile` 已增加向后兼容的 `taxMode`、`projectDiscountCents` 和 `quotationTotals`；旧数据默认 `taxExclusive`、零优惠、无冻结总额。
- 白鹅潭基准已在业务服务和真实可编辑 XLSX 回归中通过：单价 `21,200` 不变，原价 `84,800`、项目优惠 `4,900`、最终合同价 `79,900`；并明确排除 `89,888` 和 `95,281.28`。

### P0 放行结果

1. 税价模式、旧数据默认值和项目优惠已进入协议及冻结 profile/snapshot。
2. 服务层使用整数分、千分数量、bps 税率和 `i128` 中间值计算；优惠按行金额稳定分摊。
3. 文档财务汇总直接消费冻结总额，不再把已含税 `amountCents` 二次加税。
4. 白鹅潭业务计算和真实 XLSX 单元格回归已通过；优惠超过原价被拒绝。
5. 正式发布仍受 Rust 全量测试、clippy、桌面烟测、安装升级和回滚验证约束。

## 6. 当前 P0：认证与敏感数据 Host 边界

- 复核确认登录服务此前与业务 Host 权限脱节：业务读取使用硬编码 `local-operator`，写命令信任前端 `actorId`，通用 Asset 打开/导出和审批入口未绑定真实登录 principal。
- `AuthService` 已增加每次重新查库的 active principal 和 active Admin 校验；用户禁用、删除或角色降级会在下一次调用立即失效。
- 82 个 Tauri 命令已完成 active principal/Admin 门禁；9 类 command envelope 均在授权和指纹计算前由 Host 覆盖真实 `actorId/accountId`，Rust 命令入口已清除硬编码 `local-operator`。
- Member 可执行非敏感业务命令并读取递归脱敏视图；银行名称、账号、联行号和账户版本不会进入 Member 返回。AI 凭据、桌面设置、Backup、Diagnostics、资产打开/导出、Brain 永久删除和审批仍为 Admin-only。
- 记住密码已改为 Host 内部从系统 Keyring 读取并执行 `auth_login_remembered`；WebView 只接收用户名和空密码，完整密码不再经过前端 JSON。
- Admin-only 是过渡安全门，不是 1.0 最终权限模型。后续必须拆分权威 Rust Record 与 WebView View DTO，普通列表递归脱敏银行字段，并以独立、可审计 reveal/维护命令支持授权查看和更新。
- 新旧入口均已停止在未登录阶段启动项目、事件、Brain、AI 凭据和桌面设置同步；登录成功后才启动 Client SDK 全量同步，避免认证门禁落地后出现启动竞态。`pnpm run check` 已通过。
- 本轮认证后 Rust 全量库测试 `753 passed / 9 ignored`，唯一 integration suite `24/24` 通过；前端全量 `28/28` 文件、`172/172` 测试和记住密码 SDK 专项 `48/48` 通过。
- 该 P0 完成前禁止重打或分发候选安装包。

### 6.1 R2 共享成果差距复核

- 当前 R2 实现是带 SHA-256、ETag、staging、重试、崩溃恢复和 revision CAS 的异步灾备底座，不是主计划要求的共享案例库。
- `SharedCase`、案例授权、脱敏发现/预览/引用/下载、发布/撤回、同步游标和共享审计事件均未建立；后续必须复用现有 SQLite、Vault、Asset 和 R2 建立一个完整纵切，不能再造第二套存储或同步引擎。
- 当前 Asset 明确按项目去重，同 SHA 跨项目仍产生两个物理文件，与主计划“一个全局物理对象、多个项目逻辑引用”冲突；改造时必须保留历史引用并重写当前反向固化跨项目复制的测试。
- R2 共享纵切排在认证 P0 和剩余真实验收模板之后；在案例授权和真实 actor 审计完成前，不把灾备上传等同于共享成果发布。

## 7. 24 小时无人值守多窗口规则

- 总控以主计划为唯一范围；子窗口按 UI/业务、Rust/持久化、文档与测试、QA/发布拆分，禁止多个窗口同时修改同一文件或同一协议所有权。
- 每个窗口只提交可独立核验的最小增量，并回报改动文件、测试命令、结果和未决风险；总控统一集成，不把口头“完成”当作验收。
- Rust 构建固定设置 `CARGO_BUILD_JOBS=1`，避免 8 GB 内存机器并行链接导致换页和失稳；前端检查可并行，但不得与高负载 Rust 链接争抢资源。
- 每小时记录一次状态；仅失败、阻塞、P0/P1 回归、协议冲突或不可逆操作需要告警。普通成功不打断执行。
- 所有持久化改动必须保持 migration 向前、revision CAS、幂等、事件重放和重启恢复；绝对路径、凭据和敏感字段不得进入前端 JSON。
- 发布顺序固定为：P0 报价闭环 → 工作区重启/CAS 验证 → 前端全量测试与构建 → Rust fmt/check/clippy/全量测试 → 协议一致性 → 白鹅潭真实 XLSX/DOCX → 浏览器与桌面烟测 → 安装、升级、回滚。
- 任一门禁失败立即停止候选版发布，保留现场并修复根因；不得通过跳过测试、修改期望值或调整单价规避基准。

## 8. 下一执行序列

1. 完成 active principal Host 接线、真实 actor 绑定、审批和 Asset 权限基线，再拆 Member 脱敏 DTO 与银行账户独立引用/版本模型。
2. 实现视频成片验收专用 renderer：循环交付组、视频块、截图图片关系、不可拆分页和结论签章页；不得修改真实原件。
3. 实现制作成果确认的 54 镜号循环、1-3 图布局、16 页附件拼接和高亮清理决策。
4. 将剩余 2 份真实模板导入现有 Vault 并登记版本、映射和可视回归结果；不把真实原件提交 Git。
5. 用白鹅潭素材补齐前后分别回归 `4/3/1` 阻断和 5 文件 6 类内容；逐页对照历史 PDF。
6. 完成 Rust/协议/前端全量门禁、真实桌面启动、报价/验收端到端、冷启动、安装、升级、回滚、成果保全和候选包校验；不自动提交或推送。

## 9. Windows 候选包

- Codex sidecar 校验通过：`codex-cli 0.144.5`，Entrypoint SHA-256 为 `efdb3540ef74b9909408c8d38da79483454797b36f471e3e004fc2bf2b70e22a`，Authenticode 有效。
- NSIS 候选包已生成：`src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe`，大小 `131,467,452` bytes。
- 候选包 SHA-256：`D58EC91166237C88793710AC4D5E4D9C42DD2ACC7B6F9622ED8E4C7AA6A6E258`；对应 `.sha256` 文件已生成。
- 候选包当前未签名，按主计划只能作为内部包使用。
- 上述候选包生成早于本轮验收批次、协议、UI 阻断和新建项目表单改动，已不代表当前工作树；完成下一次发布门禁前不得作为业务版 1.0 当前候选版分发。
- 构建前的旧 `1.3.4` 安装包已归档为 `archive/半山商务工作台_1.3.4_pre-business-v1_20260728_x64-setup.exe`，SHA-256 为 `5841A2C7F1EFC203B5E184735217FF18AD890386BC855B4C393605A34D09AE91`。
- release 主程序隐藏冷启动 8 秒保持存活，工作集约 `40 MB`，随后由总控正常结束；未执行静默安装或覆盖系统已安装版本。

## 10. 2026-07-29 视频成片验收正式接线

- 视频成片验收专用 renderer 已通过唯一 `document_engine` 正式分发；旧 `generate_document_with_template` 入口委托空资源，新 `generate_document_with_template_and_resources` 接收经业务层水合的图片资源，不访问 SQLite 或 Vault 路径。
- `business_workspace_service` 已在数据库事务外完成视频验收资源水合，并校验冻结快照、人工确认、批次 revision、素材 binding、项目归属、Asset kind、SHA-256、PNG/JPEG MIME 与尺寸；视频只做完整性校验，截图通过现有 Vault 有界读取 helper 读取 bytes。
- 首次构建发现 `generate_document_with_template_and_resources` 尚未落地，修复后 `cargo check --lib` 通过；测试构建随后发现两个测试 initializer 缺少新增 optional 字段，已补齐并重跑。
- 文档引擎资源分发专项通过：`legacy_template_entry_delegates_with_empty_video_resources`、`resource_entry_dispatches_video_completion_renderer` 均为 `1 passed`；renderer 失败后只保留 staging 根锁文件，不残留生成子目录。
- 业务层素材安全与失败原子性专项 `video_completion_acceptance_hydration_reads_only_images_and_failures_are_atomic` 为 `1 passed`；测试 fixture 改为先建立现有 Quote 文档再冻结为验收文档，不绕过有效合同业务门禁。
- Rust 门禁：`cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 通过，`cargo check --manifest-path src-tauri/Cargo.toml --lib` 通过，相关 Rust/PowerShell/生成 TS `git diff --check` 通过。
- 独立前端验证窗口确认 `pnpm check` 通过；Client SDK `10` 个文件、`85` 个测试通过；Rust `ts(export)` 类型和生成 TS 均为 `304` 个，缺失、多余、失效 import 均为 `0`。
- 协议生成脚本的必需文件门禁已补入 5 个 `BusinessVideoCompletionAcceptance*` 类型，并逐项验证文件存在；当前生成目录与 Rust 协议不存在语义漂移。
- 专用 renderer 的真实 Word 视觉回归继续沿用本轮已核验产物：8 页、12 张图片关系、结论与双方签章同页，无标题孤页、图片/caption 分离、裁切、拉伸、重叠、溢出或空白尾页。
- 当前纵切已经恢复到可编译、专项可运行状态；Rust 全量测试、协议重新生成、正式 command 到 Asset 的完整端到端和候选包门禁尚未执行，因此不标记 1.0 发布完成，不重打或分发旧候选包。

## 11. 2026-07-29 下一纵切基线

- 视频成片验收正式接线后已补跑当前 Rust 全量库测试：`768 passed / 10 ignored / 0 failed`，耗时 `17.32s`；因此上一节“Rust 全量测试尚未执行”的风险项已关闭。
- 制作成果确认 v1 已作为下一条最高价值纵切启动，继续复用现有验收批次、Asset、Vault、SQLite、`business_workspace_service` 和唯一 `document_engine`，不建立第二套持久化或文档引擎。
- 真实空白模板只读复核：`成果确认书（制作类）.docx` 为 `23,409` bytes，SHA-256 `7F25AB4C3F1F6F92208F44CD7360717486051A351EE0202DAFCB621DA275C7BF`，Word 2021 统计为 `2` 页、`1` 个表格、`72` 个段落、`1` 个节；检查结束无残留 `WINWORD`。
- 历史回归 PDF 为 `2,385,618` bytes，SHA-256 `BD56B931D02863B9DBC515D764D704F6F61B6D29CB1F0EF9CB86C2AA0A8D5546`；视觉基线固定为主表 6 页、附脚本 16 页、4 章节、54 镜号、每镜 1-3 图。
- 并行窗口已按不冲突写集拆分：新 Rust renderer、冻结协议/TS、QA/视觉脚本；总控保留 `document_engine` 与 `business_workspace_service` 的唯一集成所有权。

## 12. 2026-07-29 制作成果确认 v1 正式接线与回归

- 制作成果确认 v1 已复用现有验收批次、SQLite、Asset/Vault、`business_workspace_service` 和唯一 `document_engine` 完成正式资源分发、冻结快照校验、人工确认、批次 revision、素材 binding、项目归属、SHA-256、PNG/JPEG MIME、图片尺寸与有界字节读取；未创建第二套数据库、任务引擎、Agent Runtime 或文档引擎。
- `document_engine` 新增制作成果资源分发专项：正确资源会进入专用 renderer；制作成果资源挂载到其他模板时返回 `BUSINESS_TEMPLATE_DATA_UNEXPECTED`，且不残留 staging 子目录。
- 业务层专项已强化为逐一核对全部 `54` 张 Vault 图片的唯一 Asset、SHA-256、文件名、MIME、尺寸和字节；未确认、stale revision、素材绑定不匹配三种失败均在各自失败后立即复核 workspace、Asset、SQLite 文件和 Vault 文件集合不变。
- 主表右侧裁切根因已修复：A4 section 可用宽度为 `9,638 dxa`，三张主表固定为 `9,400 dxa`；四张附脚本表继续使用自动宽度。未缩小字体、未删除内容、未移除 `cantSplit`，也未改变 4 章 54 镜分页策略。
- 真实注册模板 ignored 端到端通过；最新 DOCX 为 `%TEMP%\production-result-real-4596-0.docx`，SHA-256 `7DEB0F6AD84619CDC4024EABCDF0ECF9D67C138C20C91BF8ED2093AE03EAEB96`。
- Word 只读导出 PDF 为 `%TEMP%\production-result-real-4596-0.pdf`，SHA-256 `896763603AD1E4A0E1C9823DF816B5B8B103005D5659D48E2E5663F4016D3228`；Word 与 PDF 均为 `22` 页，QA 为 `PASS=64 WARN=0 FAIL=0`，输入哈希未变化，无残留 `WINWORD`。
- 人工视觉复核页：`%TEMP%\production-result-visual-4596-v1\page-01.jpg` 已确认主表完整落在页边距内、原右侧裁切消失；`%TEMP%\production-result-visual-4596-v1\page-22.jpg` 包含镜号 `54`，不是纯空白尾页。
- 首轮 Rust 全量暴露 `task_runner::tests::handler_panic_isolated_and_worker_keeps_running` 的测试竞态：测试只等待健康任务成功，panic 任务可能尚未持久化为失败。等待条件已同时要求 `Succeeded` 与 `Failed`，原有错误内容断言保留；专项连续 `20/20` 通过，`task_runner` 模块 `6/6` 通过。
- 最终 Rust 门禁：`cargo fmt --check`、`git diff --check`、`cargo check --lib` 全部通过；`cargo test --lib` 为 `784 passed / 12 ignored / 0 failed`。
- 前端与协议门禁：`pnpm check` 通过；Client SDK `10` 个文件、`85` 个测试通过；`pnpm protocol:generate` 的 Rust 导出 `309` 个测试通过，生成文件哈希无变化、无协议漂移。
- 当前自动门禁已经关闭 OOXML 安全与完整性、22 页总页数、Word/PDF 页数一致、非空尾页、主表页边距和 54 镜合成数据链路；不能据此宣布历史 `6+16` 分区视觉基线全部通过。
- 当前 ignored fixture 只有 `1` 个交付项和 `1` 张主表占位图片，实际分页仍为主表 `1` 页、附脚本 `21` 页；历史基线要求主表 `1-6` 页、附脚本 `7-22` 页。下一步必须通过现有 `business_workspace_service` 和 Vault hydration 冻结一个真实历史项目 fixture，覆盖 `3` 个交付类别、`6` 条视频证据、真实长文本、真实图片比例、4 章 54 镜，再关闭 `6+16` 逐页视觉门禁。

## 13. 2026-07-29 真实历史制作成果确认 6+16 分页基线

- 已直接从冻结历史 PDF 前 `6` 页提取并核对主表原文，不再使用简短占位标题：覆盖长视频、品牌类短视频、AIGC 类三种交付，六条证据为万象和鸣、交响共生、文脉归心、江景大家、枕月而眠、流动盛宴，并保留历史服务内容、含税价、文件名、网盘链接和提取码。
- 真实 fixture 从三份已登记脚本 DOCX 的 `word/media` 读取 PNG/JPEG，按 SHA-256 去重；六张用于主表证据，五十四张用于四章五十四镜附脚本，最终 DOCX 包含 `60` 个唯一图片 relationship，未复制真实素材到第二套产品存储。
- 主表维持 `9,400 dxa` 总宽与加宽验收图片列；第二类交付的第二条证据使用续表分页，第 `6` 页固定为验收签署区。附脚本普通镜头图片高度校准为 `1,780,000 EMU`，第四章收官序列为 `1,900,000 EMU`。
- renderer 专项 `8/8` 通过，覆盖模板哈希、恶意 ZIP/宏/外链拒绝、图片哈希和尺寸校验、原子失败、分页守卫以及真实注册模板端到端渲染。
- 最新 DOCX：`%TEMP%\production-result-real-11960-0.docx`，SHA-256 `35E7F504BB1E97789FEBBC0FCB097BF104035EDB14060500B60AADF27CC9A72D`。
- Word 只读导出 PDF：`%TEMP%\production-result-real-11960-0.pdf`，SHA-256 `55EA92AEDA606403913DAC68D34EB074726CBE36E42A2A111F84153133B333A8`；Word 与 PDF 均为 `22` 页，QA 为 `PASS=65 WARN=0 FAIL=0`，输入哈希未变化，无残留 `WINWORD`。
- 分区验证已达到主表 `1-6` 页、附脚本 `7-22` 页；第 `7` 页为第一章开篇并含镜号 `1-4` 的真实图片，第 `22` 页包含镜号 `54` 与真实图片，不是空白尾页。
- 人工视觉抽检已完成第 `1/6/7/22` 页：表格均位于页边距内，无右侧裁切、图片越界或空白尾页。第 `1` 页仍比历史件疏，交付证据从第 `2` 页开始；该差异记录为后续视觉密度优化，不阻断当前 `6+16` 分区与正式链路接线。
- 下一步将同一组 `3` 类、`6` 条证据、`54` 张脚本图片全部通过现有 `TestStore`、`asset_service::import_file`、AcceptanceBatch/material binding 和 `load_production_result_confirmation_generation_data` 水化，再执行 command 到 Asset 的正式端到端门禁；不得直接向正式 Ledger 写入伪造瑞玺记录。


## 14. 2026-07-29 真实制作成果资产正式水化门禁

- 三份已登记脚本 DOCX 的真实 PNG/JPEG 已按 SHA-256 去重并稳定排序，选取与 renderer 基线一致的前 60 个唯一图片：6 个主表证据、54 个附脚本镜头。
- 60 个图片全部通过现有测试入口中的 `asset_service::import_file` 导入同一个 TestStore Vault；未建立第二套 SQLite、Vault、任务引擎或文档引擎。
- 冻结业务数据已覆盖 3 类交付和 6 条真实成果名称：万象和鸣、交响共生、文脉归心、江景大家、枕月而眠、流动盛宴；4 章仍保持 54 镜。
- AcceptanceBatch 中建立 60 条 confirmed screenshot material binding；正式 `load_production_result_confirmation_generation_data` 逐项验证 Asset ID、SHA-256、原始名称、MIME、宽高、真实字节、项目归属、group key 和冻结 binding。
- hydration 结果通过现有 `document_engine::generate_document_with_template_and_resources` 与登记模板生成 DOCX；ZIP 验证 `word/media/` 恰好包含 60 个媒体文件。
- 首轮测试暴露 Windows 文件锁：`StagedDocument` 的 generation lease 尚未释放时，原子性快照读取 staging 文件失败。修复为验证生成包后显式释放 ZIP 与 staged lease，再继续失败原子性断言。
- 专项命令在固定 Rust 资源设置下通过：`CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`；结果为 `1 passed / 0 failed`，其余 `795` 个库测试被过滤，执行约 `9.13s`。
- 新测试：`production_result_confirmation_hydrates_real_60_assets_generates_docx_and_rejects_stale_or_mismatched_state_atomically`。它同时保留人工确认、过期 revision、group mismatch 与数据库/Vault 不变的原子失败门禁。
- 当前剩余缺口：测试仍直接登记 AcceptanceBatch/material rows；下一纵切必须改用现有 CreateAcceptanceBatch、UpsertAcceptanceMaterial、PrepareAcceptanceDocuments、状态审批和 GenerateDocument 命令，将生成结果验证为最终 Vault DOCX Asset。

## 15. 2026-07-29 正式命令链、全量门禁与内部试用包

- 已将真实制作成果确认门禁从“直接登记测试表行”升级为正式业务命令链：`prepare_effective_contract → CreateAcceptanceBatch → 60 × UpsertAcceptanceMaterial → PrepareAcceptanceDocuments → Draft/InReview/Approved → GenerateDocument`。每次素材写入都使用最新 workspace revision，不绕过现有命令回执、CAS、事件、Vault 或文档引擎。
- 冻结快照验证为 60 条 material binding、3 类交付、6 条主表证据、4 章、54 镜；正式生成前后只新增 1 个最终 DOCX Asset，来源为 `BusinessDocument`、项目归属正确、Ledger 只链接一次，Vault DOCX 的 `word/media/` 恰好 60 个文件。
- 真实外部素材门禁保留为带原因的 ignored 测试，必须显式提供本机登记模板和三份脚本 DOCX；本轮显式执行结果为 `1 passed / 0 failed / 795 filtered out`，约 13.18 秒。
- Rust 全量首次回归暴露视频验收测试缺失测试文档，导致 `workspace.documents[0]` 越界；已恢复该测试原有 `create_test_document`，专项结果 `1 passed / 0 failed`。
- Clippy 首轮发现 11 项新增告警：付款模板 7 处 needless borrow、2 处整数倍数手写判断、视频验收 1 处布尔比较、模板审核函数 1 处参数过多。已分别改为直接切片引用、`.is_multiple_of()`、逻辑否定和 `TemplateVersionReview` typed request，不使用全局 allow 掩盖。
- 当前 Rust 门禁全绿：`cargo fmt --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings` 通过；库测试 `783 passed / 13 ignored / 0 failed`，integration `24 passed / 0 failed`。
- 当前前端门禁全绿：`28/28` 个测试文件、`172/172` 项测试通过，`pnpm check` 和生产构建通过。业务技能包为 `14 skills / 14 tools / 35 files`。
- `pnpm release:verify` 全链通过：协议导出 `309/309`，生成前后协议 manifest 共 310 个条目且 diff 为 0；Codex sidecar `0.144.5`、签名和 manifest 校验通过；`git diff --check` 退出码为 0。
- 制作成果确认结构 QA 重跑为 `PASS=61 WARN=0 FAIL=0`；DOCX SHA-256 仍为 `35E7F504BB1E97789FEBBC0FCB097BF104035EDB14060500B60AADF27CC9A72D`，PDF SHA-256 仍为 `55EA92AEDA606403913DAC68D34EB074726CBE36E42A2A111F84153133B333A8`。
- 已将 PDF 1-22 页全部渲染为六张 contact sheet 并逐页人工检查：主表 1-6 页、附脚本 7-22 页分区正确；四章和镜号 1-54 连续；表格均在页边距内，表头、图片、文字和边框可见，无图片越界、右侧裁切、中间空白页或空白尾页。此前仅抽检 1/6/7/22 页的视觉缺口已关闭。
- 在同一工作树、同一内部模型配置下重新执行 `pnpm release:build:internal`，其内置 release verify 再次全绿，并生成当前源码对应的 Windows NSIS 内部试用包：`src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe`。文件大小 `132,036,686` bytes，生成时间 `2026-07-29 20:15:46 +08:00`，SHA-256 `4A604B15F5FD75ABD4D450471F0F539E01BE63207B8E4D91DED39940F85C4580`。
- 当前包未做 Authenticode 产品签名，只能作为内部试用包，不得对外发布；旧的 `D58EC...` 安装包已被新构建替代。
- 阶段判断：已到主计划第 14 天“白鹅潭端到端测试和内部试用包”代码与产物节点，但第 14 天仍缺两台 Windows 机器的安装/升级/冷启动实测。它不是第 20-21 天 1.0 候选版；R2 共享案例/去重/管理员权限、年框结算、基础法务、联网搜索、macOS、升级回滚和两机 20 次冷启动仍属于后续硬门禁。
- 当前工作树 `main / 206bc83` 共 79 项状态（32 项已跟踪修改、47 项未跟踪），均未提交；不得把未提交工作树或内部未签名包表述为稳定 1.0。

## 16. 2026-07-29 本机安装升级门禁与第 15 天启动

- 已增强 `scripts/invoke-nsis-release-acceptance.ps1`：新增 `-ColdStartCount`，默认和最小值均为 2；每轮冷启动均验证应用保持运行、PID 独立、正常退出、无残留测试进程，以及 SQLite/Vault 哨兵继续存在，验收摘要同步记录 `coldStartCount`。
- 使用全新隔离 RunId `business-v1-day14-local-20-cold-starts` 完成本机 Windows 11 专业版 Build `26200` 门禁：初始安装通过、同版本覆盖升级通过、`20/20` 次连续冷启动通过、SQLite/Vault 每轮保留、卸载通过、HKCU 卸载注册表恢复通过。
- 本机验收摘要保存在 `.runtime/nsis-acceptance/business-v1-day14-local-20-cold-starts/acceptance-summary.json`；验收后的 `半山商务工作台` 卸载注册项和 `bsaigc_desktop` 运行进程均为 0，未保留测试安装残留。
- 发现内部构建完成后配套 `.sha256` 仍保留旧包值 `D58EC911...`，与当前安装包不一致；已修复 `scripts/build-internal-preview.ps1`，构建后使用 .NET 流式 SHA-256 自动重写校验文件，避免发布校验再次陈旧。
- 当前安装包 `src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe` 大小为 `132,036,686` bytes，实际 SHA-256 和配套校验文件均为 `4A604B15F5FD75ABD4D450471F0F539E01BE63207B8E4D91DED39940F85C4580`；两个修改后的 PowerShell 脚本均通过语法解析。
- 启动真实用户数据前已保存关键目录快照 `%LOCALAPPDATA%\Temp\bsaigc-preinstall-critical-20260729-202112`，包含 ledger、vault、credentials、brain-workspace、staging 和 third-party；完整 AppData 复制因 codex-home 超长路径失败，该失败副本不作为恢复依据。
- 阶段判断更新：第 14 天的代码、产物和当前机器安装/覆盖升级/20 次冷启动已闭环，主线正式进入第 15-17 天“R2 共享案例、SHA 去重、本地授权索引和管理员权限”。第二台 Windows、独立 Windows 测试用户、macOS、产品签名和升级回滚仍是外部门禁，不能宣称第 20-21 天 1.0 候选版完成。
- 共享案例审计确认继续复用同一 SQLite、Vault、Asset、BackupOutbox、现有 R2 transport 和管理员认证；首个纵切只实现发布、授权、撤回、幂等回执、revision CAS 和授权列表，不把私有自动备份误当作共享发布，也不创建第二套数据库、R2 客户端或任务引擎。

## 17. 2026-07-29 R2 共享案例、SHA 去重与授权索引纵切

- 本纵切继续复用同一 SQLite、Vault、Asset、BackupOutbox、现有 R2 worker、Auth 和 Client SDK；只新增共享成果 publication、grant、event 和 command receipt 索引，没有创建第二套数据库、R2 客户端、Agent Runtime、任务引擎或文档引擎。
- 已实现管理员发布、授权原子替换和撤回；命令支持 revision CAS、稳定 command ID、幂等 receipt replay 和请求 fingerprint 冲突检测，所有状态变更均写入同一事务并追加 durable event。
- 发布前强制验证 Vault 文件和登记 SHA-256 一致。BackupOutbox 已存在相同 SHA-256 成功对象时直接复用远端 Key/ETag；未命中时使用稳定 UUID v5 和稳定幂等键排队，随后唤醒现有 backup worker，不重复实现上传或重试系统。
- 已增加 `pendingBackup → published` 幂等收敛：授权读取前查询 BackupOutbox 成功对象，原子回填远端 Key/ETag、状态、revision 和更新时间，并追加一次 `published` durable event；重复读取不会重复追加事件。
- 未完成 R2 上传的 `pendingBackup` 案例不会暴露给被授权普通用户；授权发现接口只返回 `published`。原发布命令的 receipt 保留首次响应快照，最新状态通过授权列表和事件流获取，确保命令重放语义稳定。
- 已新增 `replay_shared_case_events` Tauri command，并贯通 HostAdapter、DesktopHostAdapter 和 BsaigcClient；支持 `afterSequence` 增量回放和 `1..=1000` limit 校验，Host 能力缺失和 SDK 参数错误均有明确失败结果。
- SharedCase 专项测试 `9 passed / 0 failed`，覆盖 pendingBackup、SHA 复用、pending→published 收敛、授权过滤、事件回放 limit、授权 CAS/原子替换、撤回隐藏、receipt replay/fingerprint 冲突和 Vault 篡改拒绝；BackupOutbox SHA 专项 `3 passed / 0 failed`。
- Rust 全量库测试 `806 passed / 13 ignored / 0 failed`，共 819 项；协议导出与 bindings 校验 `320 passed / 0 failed`；Client SDK 为 `2 files / 50 tests passed`。
- `cargo check --lib`、`cargo clippy --lib -- -D warnings`、`pnpm check` 和 `git diff --check` 全部通过；Rust 构建统一使用 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0` 和 `CARGO_INCREMENTAL=0`。
- 本纵切只补齐 Host/SDK 和持久化能力，没有新增业务页面，因此没有新的 UI 视觉差异，视觉验收不适用；后续管理员管理界面只能接入 `src/business-v1`，不得扩建旧 `App.tsx` 或旧业务页面。
- 阶段判断：第 15-17 天共享案例、SHA 去重、本地授权索引和管理员权限的核心服务纵切已闭环，但尚未完成第二台 Windows、独立 Windows 用户、macOS、Authenticode、正式升级回滚，以及主计划第 18-19 天年框结算、基础法务和联网搜索，因此仍不能宣称 1.0 候选版完成。



## 18. 2026-07-29 新业务壳共享案例管理员闭环

- 本轮继续严格限定在 `src/business-v1` 新业务壳，未扩建旧 `App.tsx` 或旧业务页面；前端直接复用现有 `BsaigcClient`、SharedCase Host 命令、SQLite/Vault/Asset/BackupOutbox/R2 链路，没有创建第二套数据库、R2 客户端、Agent Runtime、任务引擎或文档引擎。
- 已完成共享案例中心的用户可见闭环：管理员可从当前项目选择本地案例发布、编辑授权并撤回；普通用户只看到授予 `discover` 权限且已完成云端发布的案例列表。发布候选严格过滤为 `caseRecord.projectId === activeProjectId`，并排除已有 `pendingBackup` 或 `published` publication 的案例；已撤回案例允许重新发布。
- 管理员打开共享案例中心时会额外调用 `replaySharedCaseEvents(0, 1000)`，显示最后 durable event sequence，便于确认发布、授权和撤回是否已进入审计流；普通用户不会调用事件回放。账号切换后，默认管理员授权文本会同步刷新为当前用户名。
- 修复事件回放授权边界：Tauri `replay_shared_case_events` 从仅要求活动登录改为 `require_active_admin()`。普通用户继续通过 `list_authorized_shared_cases` 获取服务端过滤后的快照，不能通过全量 publication/grant 事件绕过授权发现边界。
- 共享案例 UI 专项测试为 `8 passed / 0 failed`，覆盖授权文本解析、管理员 discover 保底、普通用户不显示/不调用管理能力、管理员最后事件序列展示、当前项目候选过滤、未撤回 publication 排除及撤回后重新发布。
- 前端全量回归为 `29 files / 182 tests passed`；`pnpm check`、`pnpm build` 和 `git diff --check` 全部通过。生产构建完成 1650 个模块转换。
- Rust 共享案例专项为 `9 passed / 0 failed`；Rust 全量库测试仍为 `806 passed / 13 ignored / 0 failed`，共 819 项；`cargo check --lib` 与 `cargo clippy --lib -- -D warnings` 通过。Rust 命令继续统一使用 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_INCREMENTAL=0`。
- 已使用真实生产构建完成桌面宽屏和 620px 窄屏视觉验收：弹层层级、标题区、刷新/关闭按钮、错误/成功提示、案例卡片、权限标签、状态徽标和窄屏单列布局均可读；WebHost 下“当前运行环境不支持读取共享案例”属于预期能力降级，普通用户视图未暴露发布、授权、撤回或事件序列。管理员交互由 React 专项测试覆盖，真实数据链由 Rust/SDK 回归覆盖。
- 阶段判断：主计划第 15-17 天“R2 共享案例、SHA 去重、本地授权索引和管理员权限”现已同时完成服务、Host/SDK、权限边界、新业务壳 UI、自动测试和视觉验收闭环。下一最高价值纵切进入第 18-19 天年框结算；基础法务与联网搜索随后推进。第二台 Windows、独立 Windows 用户、macOS、Authenticode、正式升级回滚仍是 1.0 候选版外部门禁，因此当前仍不能宣称 1.0 候选版完成。

## 19. 2026-07-29 年框结算全链闭环

- 本纵切按主计划第 18-19 天推进，继续只扩展 `src/business-v1` 新业务壳；未扩建旧 `src/App.tsx`，未创建第二套 SQLite、Vault、Agent Runtime、任务引擎、文档引擎或 R2 客户端。服务端复用现有 `business_workspace_service`、命令回执、CAS revision、durable event 和 Client SDK。
- 已完成 SettlementBatch 协议、SQLite 持久化、Host 权限、命令回执、事件投影和 SDK/UI 全链闭环；支持月度、季度、按单、一次性和混合口径，创建/更新与作废均使用 workspace revision CAS 和稳定幂等上下文。作废会释放交付项，未作废批次会阻止同一交付项被另一批次重复引用。
- Rust 回归覆盖连续两个季度和一次单独结算且不重复引用交付项；补充覆盖无效数量 `BUSINESS_SETTLEMENT_QUANTITY_INVALID`、过期 revision `REVISION_CONFLICT`、幂等键碰撞 `IDEMPOTENCY_KEY_REUSED`，并验证所有失败路径对持久化 workspace 无副作用。
- 协议生成与 binding 导出校验为 `328/328` 通过；Client SDK 已增加 `upsertBusinessSettlementBatch` 和 `voidBusinessSettlementBatch`，并覆盖正式 envelope、CAS revision、command/idempotency context 与事件 projection。
- 新业务壳已接入“发起结算”独立任务入口和 `AnnualSettlementCenter`：展示项目、有效批次、可结算交付项、历史批次和作废态，支持新建、编辑和作废；WebHost 创建真实项目时显示 `WebHostAdapter only reserves the HTTPS/WebSocket protocol mapping and is not implemented. Use DesktopHostAdapter.`，属于预期能力降级，真实数据链由 Rust 与 SDK 回归覆盖。
- 年框结算专项前端测试为 `4 files / 68 tests passed`；前端全量为 `32/32 files / 200/200 tests passed`，`pnpm check` 与 `pnpm build` 通过，生产构建完成 1652 个模块转换。
- Rust 门禁全绿：`cargo fmt -- --check`、`cargo check --lib`、`cargo clippy --lib -- -D warnings` 通过；全量库测试为 `815 passed / 13 ignored / 0 failed`，共 828 项。Rust 构建统一使用 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0` 和 `CARGO_INCREMENTAL=0`。
- 已使用真实 `AnnualSettlementCenter` 组件完成 1440×960 宽屏与 620×900 窄屏视觉验收：宽屏双栏表单/批次布局、窄屏单栏折叠、项目标题与统计、交付项数量信息、批次卡片、确认/作废徽标、作废按钮、空批次提示和表单错误提示均可读；未发现遮挡、横向溢出、按钮不可达或关键状态不可辨识问题。静态审计另记录非阻断硬化项：长项目名头部换行、宽屏双滚动区、忙碌态提示与 `aria-busy`、焦点锁定/恢复、`Escape` 关闭，以及作废卡片整体透明度对正文对比度的影响。
- 阶段判断：年框结算的 Service/Host/SDK/UI、自动回归和视觉验收现已闭环，满足主计划验收项“连续运行两个季度和一次单独结算，不重复引用已结算交付项”。主计划第 18-19 天仍剩基础法务与联网搜索；第二台 Windows、独立 Windows 测试用户、macOS、Authenticode 和正式升级回滚仍是外部门禁，因此当前仍不是 1.0 候选版。

## 20. 2026-07-29 基础法务全链闭环

- 本纵切继续只扩展 `src/business-v1` 新业务壳，未扩建旧 `src/App.tsx`；合同审查中心只通过现有 `BsaigcClient` 调用既有合同审查 Host 能力，复用 SQLite、Vault、`business_workspace_service`、`document_engine`、OCR、任务事件和资产打开能力，没有创建第二套数据库、Agent Runtime、任务引擎或文档引擎。
- 已完成 `ContractLegalCenter` 及响应式样式，并接入新业务壳“合同审查”独立快捷入口；项目切换会关闭法务中心，附件候选从当前项目已导入资产和现有业务文档资产中按资产 ID 去重，仅接受 PDF/DOC/DOCX，打开原件与报告继续复用现有资产能力。
- 法务中心覆盖审查列表、创建并启动、事件订阅刷新、阶段重试、风险清单、Evidence 原文定位、逐条人工决策和 DOCX 报告门禁。旧报告存在时不会绕过当前 revision 的未决风险门禁；全部 Finding 完成人工决策后才允许生成新报告。
- WebHost 明确显示“不支持合同审查”的能力降级，不伪造创建、决策或报告成功；真实写操作、OCR、Vault 资产与 DOCX 输出由 DesktopHost/Rust 专项回归覆盖。
- 法务专项前端测试为 `2 files / 10 tests passed`；前端全量为 `34/34 files / 210/210 tests passed`，`pnpm check` 与 `pnpm build` 通过。
- Rust 合同审查专项为 `29 passed / 0 failed`，覆盖真实 DOCX 的低风险、高风险、缺字段、规则、Agent 降级、Evidence、CAS、事件回放和报告链路；构建统一使用 `CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0` 和 `CARGO_INCREMENTAL=0`。
- 修复内部预览认证 fixture 对 `auth_remembered_credentials`、`auth_remember_credentials`、`auth_forget_credentials` 的缺口，并补齐预览业务工作区新增的 `templateVersions`、`settlementBatches`、`acceptanceBatches` 字段；登录后新业务壳不再因缺失结算数组出现空白页。
- 已完成 1440×960 宽屏与 620×900 窄屏真实浏览器视觉验收，截图位于 `docs/visual/legal-center-1440x960.png`、`docs/visual/legal-center-620x900.png` 和 `docs/visual/legal-center-620x900-detail.png`。两种视口均满足 `document.scrollWidth <= innerWidth`，弹层边界完整位于视口内，标题、刷新、关闭、指标、风险列表、Evidence、人工决策与报告门禁无遮挡或按钮裁切；窄屏滚动到底后可访问第 5 页 Evidence、原文、建议条款、已保存“要求修改”决策及禁用的 DOCX 报告按钮，控制台无错误。
- 阶段判断：主计划第 18-19 天的年框结算与基础法务现已闭环，下一最高价值纵切进入联网搜索与来源记录。第二台 Windows、独立 Windows 测试用户、macOS、Authenticode 和正式升级回滚仍是 1.0 候选版外部门禁，因此当前仍不能宣称 1.0 候选版完成。
## 21. 2026-07-29 联网搜索与来源记录全链闭环

- 本纵切只扩展 `src/business-v1`，未扩建旧业务界面；联网能力复用 Codex Runtime 内置 Web Search、现有 Brain Host、SQLite turn 持久化和现有 Client SDK，没有创建第二套搜索客户端、数据库、Agent Runtime、任务引擎或文档引擎。
- `webEnabled` 默认或缺省为 `false`，只有用户显式选择允许联网时才启用 `live` Web Search，并将现有 WorkspaceWrite 的 `network_access` 设置为 `true`；本地模式、旧客户端请求和非法范围均保持禁网，FullAccess 的既有权限语义不变。
- Host 联网安全策略明确限制为公开信息检索：不得把附件原文、合同原文、银行账号、客户秘密或本地路径作为搜索查询发送；来源必须包含 URL 与访问日期并标记“外部未确认”；外部结果不得自动覆盖公司、合同或项目正式数据。
- 来源记录复用已持久化的 `BrainTurnRecord.assistantText`：提取公开 `http/https` URL，规范化并去重，过滤 `localhost` 与私网地址；来源卡片展示标题、域名、访问日期、“外部未确认”和“不覆盖正式数据”，并使用安全的新窗口打开方式。
- 预览 fixture 已加入 W3C 官方公开 URL 的联网 assistant turn，用于来源卡片、域名、长 URL、访问日期和未确认标签的真实界面验收。
- 联网专项前端测试为 `5 files / 24 tests passed`；`pnpm check` 与 `git diff --check` 通过；Rust `brain_host` 专项为 `15/15 passed`，协议缺省兼容专项为 `11/11 passed`。
- 本轮未执行真实外网搜索调用，仅完成运行时配置链路、权限边界、来源投影、持久化读取和预览验收准备；真实效果仍依赖 Codex Runtime 登录状态与网络环境。
- 阶段判断：主计划第 18-19 天的联网搜索与来源记录现已闭环。下一阶段进入第 20-21 天候选版构建、数据迁移、升级与回滚门禁；第二台 Windows、独立 Windows 测试用户、macOS、Authenticode 及正式升级回滚验证完成前，仍不宣称 1.0 候选版完成。

## 22. 2026-07-29 Windows 候选包实装与发布归档门禁

- 本章节的权威事实日期为 `2026-07-29`。验收摘要受宿主机时钟异常影响显示为 `2026-07-30 +08:00`，该时间仅作为时钟异常披露，不改变本轮构建、验收与归档事实归属日期。
- `pnpm release:verify` 已通过：前端 `36 files / 217 tests`，Rust `818 passed / 13 ignored`，tools `24 passed`；Codex sidecar 版本为 `0.144.5`，其签名状态为 `Valid`。
- 本轮安装包为 `src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe`，大小 `132211762 bytes`，SHA-256 为 `176e3846199411a4a515fdacfdbb152ac541acd0d236ea48e6a6d8ca6307f5b3`，Authenticode 状态为 `NotSigned`；候选包未内置 API Key，未内置 R2 凭据。
- NSIS 验收 runId 为 `windows-rc-134-20260729-a`：首次安装、`same-package-reinstall`、连续 `20` 次冷启动、SQLite/Vault/credentials/brain-workspace/business-workspace 哨兵、卸载后数据保留及注册表恢复均通过。`same-package-reinstall` 仅证明同版本覆盖安装链路通过，不等同于真实跨版本升级验证。
- 打包过程中完成三项门禁修复：`scripts/package-release.ps1` 修复 Windows PowerShell 5.1 对 UTF-8 无 BOM 中文字面量的解析问题；新增 `scripts/invoke-windows-rc-package.ps1`，在发布打包期间临时精确清空 R2 凭据并通过 `finally` 原样恢复；`scripts/create-source-snapshot.ps1` 修复中文 ZIP entry 使用 UTF-8 创建后回读乱码的问题。
- `release/1.3.4` 已包含 `8` 个标准产物；source snapshot 共 `1526 files`，扫描结果为 `0 sensitive findings`。安装包实算、build manifest、NSIS acceptance summary 与 release 归档校验清单的四方 SHA-256 绑定一致；R2 配置已原样恢复且无备份残留。旧含凭据归档已移动至 `release/1.3.4-legacy-embedded-credential-archive-20260728`。
- 当前仍不能宣称 1.0 候选版完成：真实跨版本升级与回滚、第二台 Windows、独立 Windows 用户、macOS 签名/公证/启动验证以及 Authenticode 签名均未完成，必须继续作为发布门禁推进。

## 23. 2026-07-29 真实跨版本升级与隔离回滚门禁闭环

- 本章节全部以 `2026-07-29` 作为权威事实日期，不采用宿主机验收日志显示的 `2026-07-30` 日期作为本轮事实归属日期。
- 修复 `scripts/invoke-nsis-release-acceptance.ps1` 的三个门禁假阴性：兼容 Windows PowerShell 5.1 的 rollback manifest 数组扁平化；冷启动实例标识改为 `PID + StartTime ticks`，不再要求 PID 全局唯一；卸载后的注册表清理改为幂等校验，已不存在的目标项不会被误判为失败。
- 成功真实跨版本摘要已归档为 `release/1.3.4/upgrade-acceptance-summary.json`，SHA-256 为 `86da82536f09b133f1a7933f09da260433081c746911ef9327d22d592b2daf10`；`runId=cross-version-133-to-134-20260729-b`，`status=passed`，`version=1.3.4`，`upgradeKind=cross-version-upgrade`，完成 `1.3.3 → 1.3.4`；`coldStartCount=20`，`preflight`、`initial-install`、`data-backup`、`first-start`、`upgrade`、`restart`、`uninstall`、`registry-restore` 全部为 `passed`；`backupCreated=true`、`rollbackAttempted=false`。
- 成功摘要绑定的当前安装包大小为 `132211762 bytes`，SHA-256 为 `176e3846199411a4a515fdacfdbb152ac541acd0d236ea48e6a6d8ca6307f5b3`，Authenticode 为 `NotSigned`；上一版本安装包 SHA-256 为 `c72692fe8fc13368575d1936cc0f213c10cbde3b1c69b8b93b62db7cfcf59da0`。
- 故障注入回滚摘要已归档为 `release/1.3.4/rollback-acceptance-summary.json`，SHA-256 为 `6019399a897bcc658505316bddf304bf74fcfa37f211cf72f9eeeab6facf1024`；`runId=cross-version-133-to-134-rollback-20260729-b`，`status=failed` 但属于预期注入失败；`injectFailureAfterUpgrade=true`，`error=Injected failure after overwrite upgrade to verify isolated data rollback.`；`backupCreated=true`、`rollbackAttempted=true`、`rollbackCompleted=true`、`rollbackError=null`、`uninstallCompleted=true`、`registryRestored=true`；`preflight`、`initial-install`、`data-backup`、`first-start`、`upgrade`、`data-rollback` 全部为 `passed`，`acceptance` 为 `failed`。
- 故障注入后，SQLite、Vault、credentials、brain-workspace、business-workspace 哨兵均恢复；回滚前后文件集合、文件大小和 SHA-256 与升级前快照一致，证明隔离数据回滚闭环有效。
- 发布归档已重建：`release-manifest.files=10`，`SHA256SUMS.txt=11` 条记录，全部实算匹配；旧的 `cross-version-133-to-134-20260729-a` 失败摘要未归档进 `release/1.3.4`。
- 阶段判断：主计划第 20-21 天的 Windows 构建、数据迁移、真实跨版本升级与隔离回滚门禁已完成；第二台 Windows、独立 Windows 用户、macOS 签名/公证/启动验证以及 Authenticode 签名仍未完成，因此当前仍不能宣称 1.0 RC 完成。

## 24. 2026-07-29 第二台 Windows 可携带验收与发布前校验门禁

- 本章节全部以 `2026-07-29` 作为权威事实日期。本轮发现 `release/1.3.4` 中的候选安装包已归档为英文文件名 `banshan-workbench-v1.3.4-setup-unsigned.exe`，无法直接通过现有 `scripts/invoke-nsis-release-acceptance.ps1` 强制要求的 `半山商务工作台_1.3.4_x64-setup.exe` 中文文件名门禁。
- 新增 secondary machine bundle 生成器、目标机 runner 和配套操作文档；生成器先复算 `release-manifest.json` 与 `SHA256SUMS.txt`，再在隔离携带包目录中复制并转换为验收引擎要求的中文安装包名称，不修改 `release/1.3.4` 原安装包。目标机 runner 继续复用 `scripts/invoke-nsis-release-acceptance.ps1`，没有创建第二套 Windows 安装验收引擎。
- 新增脚本均通过 Windows PowerShell 5.1 Parser 检查；生成器 `-DryRun` 和完整 ZIP 生成均通过。完整 ZIP SHA-256 为 `b5d37c4301d54d37f16674630cffa2dff44b910ee949824d1220ce2ff6c9cbad`，`bundleFiles=6`。
- 目标机 runner 已在本机构建机显式关闭 machine/user 差异门禁后完成 `Mode Both -DryRun`：跨版本升级和故障注入回滚两个分支均成功进入既有 NSIS 验收流程，携带包内 `6` 项文件校验通过，候选安装包 Authenticode 状态为 `NotSigned`。该结果只证明携带包、名称转换、校验绑定和执行入口可运行，不代表第二台机器或独立用户已完成真实验收。
- 新增 `scripts/verify-release-candidate.ps1`，并接入 `.github/workflows/build-windows.yml` 的产物上传前门禁；本机已使用该脚本对 `release/1.3.4` 发布归档及 NSIS acceptance summary 完成验证并通过，用于在上传前阻止安装包、manifest、校验清单或验收摘要绑定不一致的候选产物。
- 本轮没有虚构第二台 Windows 或独立 Windows 用户的执行结果。外部门禁仍包括：在不同 Windows MachineGuid 的第二台机器上使用不同用户 SID 完成真实 `Both` 验收并回传证据；完成 macOS 构建、签名、公证、真实启动和升级验证；完成 Windows Authenticode 产品签名。在这些门禁关闭前，仍不能宣称 1.0 RC 完成。

## 25. 2026-08-02 旧业务界面下线与 1.0 唯一入口

- 依据用户最新决策和主计划“新业务壳 + 复用成熟底层”策略，完成旧业务 UI 下线：删除 `src/App.tsx`、`src/App.css`、`?legacy=1`、`legacyTarget` 和旧版返回按钮。根入口 `RootApp` 现在无条件渲染 `BusinessV1App`。
- 进一步建立 TypeScript/CSS 生产可达依赖图：1.0 入口可达 `362` 个模块，识别出 `32` 个仅属于旧壳且无生产或共享底座外部引用的源码文件；连同 `11` 个专属测试共物理删除 `43` 个文件。删除范围包括旧 `BusinessWorkbench`、`DesktopShell`、`BusinessDocumentsCenter`、`BrainCenter`、`TaskCenter`、`AssetVault`、`CaseLibrary` 及其专属 helper/CSS，不删除新版复用的登录、设置、Client SDK 或 Rust Host 代码。
- 新增 `src/business-v1/useBusinessSettingsController.ts`，复用现有 `BsaigcClient`、AI 凭据、Desktop Settings、SQLite/Vault 路径、缓存清理、R2 状态和更新 API；未创建第二套数据库、Vault、Agent Runtime、任务引擎或文档引擎。
- 侧栏“设置”直接打开 AI 服务分类，“检查更新”直接打开“更新与关于”；修改密码、管理员用户管理、共享账号刷新和退出登录均在 1.0 弹窗内完成，不再卸载当前项目工作区。
- 修复 Base URL 输入和粘贴的状态快照问题：事件触发时立即保存输入值，并覆盖 `input`/`change`，避免界面显示有值但保存校验读取空串。
- 旧数据兼容继续保留：SQLite Ledger、Local Vault、历史表、历史文件、迁移、`business_workspace_service`、R2、OCR、文档引擎、Task/Brain/Codex Runtime 和发布链路均未删除或重建。
- 兼容边界已单独审计：SDK/后端仍可读取历史任务、案例、需求简报、执行简报、资产和归档会话，但 1.0 当前尚未为其中全部数据提供普通用户可见的只读入口。这是“可见性缺口”而非数据丢失；后续必须在新壳内补齐只读兼容入口，不恢复旧 UI。
- 物理清理后全量前端 `26` 个测试文件、`175/175` 测试通过，TypeScript 检查、生产构建和 `git diff --check` 通过；测试减少量全部来自已删除且生产不可达的旧 UI 专属测试。
- 真实浏览器冒烟通过：普通地址和 `?legacy=1` 均只显示“半山商务工作台 1.0”，页面无“旧版/返回新版/legacy”入口或按钮，控制台无 warning/error。
- 生产构建通过，输出单一 1.0 JavaScript bundle `dist/assets/index-HMFaPoPA.js`（`428.90 kB`，gzip `121.92 kB`），不生成旧 App 或旧壳 chunk；生产 CSS 中旧壳根选择器均为 `0` 次。

## 26. 2026-08-02 旧版彻底下线复核与历史资料入口收口

- 产品运行入口已复核为单一路径：`src/main.tsx` 只挂载 `RootApp`，`RootApp` 无条件渲染 `BusinessV1App`；`src/` 中不存在旧 `App`、`BusinessWorkbench`、`DesktopShell`、`BusinessDocumentsCenter` 或 `legacy` 路由的生产引用。
- 清理面向用户的迁移措辞：历史资料中心不再展示“新版/旧版”说明，会话置顶未开放提示也不再引用“新版索引”。“新版本”仅保留在软件更新和文档版本语义中，不代表第二套产品界面。
- 新壳侧栏保留唯一“历史资料”入口，集中读取现有任务、案例、需求简报、执行简报、通用资产和归档会话；未恢复任何旧页面、旧路由或旧业务组件，未创建第二套 SQLite、Vault、任务引擎、Agent Runtime 或文档引擎。
- 历史资料中心补齐 Escape 关闭、搜索、分类计数、当前项目过滤、刷新、资产打开、空态和错误态；归档/恢复会话后 `BsaigcClient` 立即更新 `BrainProjection` 并发布快照，避免界面停留在旧状态。
- 新增和修复历史资料 ViewModel、界面及 Client SDK 回归测试；专项 `57/57` 通过，全量前端 `28` 个测试文件、`181/181` 通过，`pnpm check`、`pnpm build`、`git diff --check` 均通过。
- 生产产物重新生成：`dist/assets/index-CdqMy2lU.js`（`439.06 kB`，gzip `124.83 kB`）和 `dist/assets/index-BXZegSOf.css`（`97.58 kB`，gzip `15.92 kB`）；产物扫描未发现 `legacyTarget`、`?legacy=1`、旧工作台组件名、旧版入口或返回新版标记。
- 真实浏览器冒烟通过：普通地址和带 `?legacy=1` 的地址标题均为“半山商务工作台 1.0”，页面旧版标记为 `0`，历史资料按钮唯一，弹层可打开并由 Escape 关闭，浏览器控制台 warning/error 为 `0`。
- `release/1.0.0` 至 `release/1.3.4` 目录中的旧安装包仅作为跨版本升级与回滚证据保留，不属于当前运行入口，也不得作为业务版 1.0 当前候选版分发；正式发布必须基于当前源码重新生成全新 Windows/macOS 包、manifest 和 SHA-256。

## 27. 2026-08-02 旧版下线发布级收尾

- 再次按主计划复核删除边界：旧业务界面、旧入口、旧路由和旧界面专属 helper/test 可以删除；SQLite、Vault、历史表、历史文件、迁移、`business_workspace_service`、`document_engine`、R2、OCR、Task/Brain/Codex Runtime 及旧模板兼容器必须继续保留。
- 清理发布源码残影：删除未跟踪的 `tsconfig.tsbuildinfo`、`tsconfig.node.tsbuildinfo`、`vite.config.js`、`vite.config.d.ts`，其中 TypeScript build info 已确认包含旧 UI 文件路径；`.gitignore` 增加对应规则，后续构建不会再把这些缓存或配置转译文件带入源码快照。
- 清理后重新执行 `pnpm check`、`pnpm build` 和 `git diff --check` 均通过；构建未重新生成上述四个文件，生产 `dist` 扫描中 `legacyTarget`、`?legacy=1`、`BusinessWorkbench`、`DesktopShell`、`BusinessDocumentsCenter`、旧版入口、返回新版标记均为 `0`。
- 真实浏览器复核：普通地址和 `?legacy=1` 都只进入“半山商务工作台 1.0”；旧版标记为 `0`，唯一“历史资料”入口可打开并关闭，浏览器控制台 warning/error 为 `0`。
- 源码快照 Dry Run 通过：包含 `1495` 个文件、排除 `44` 项、敏感信息命中 `0`，最终快照指纹为 `b73a6db8fb926d9059144c33c77172b0beeabc3352ed7f6a2858426f4c9125d0`。
- 当前工作树中的旧 UI 已物理删除，但 `HEAD` 与 `origin/main` 仍为删除前提交；因此不得从当前远端提交或历史 `release/1.3.1` 至 `release/1.3.4` 包宣称“旧版已下线”。正式候选版必须先提交当前删除和 1.0 新壳，再从干净检出重新构建 Windows/macOS、源码 ZIP、manifest 与 SHA-256；历史安装包只保留为离线升级/回滚证据，不再分发。

## 28. 2026-08-02 桌面二进制重建与唯一入口最终验收

- 在 Rust 构建统一使用 `CARGO_BUILD_JOBS=1` 的前提下，执行 `pnpm tauri build --no-bundle` 完成当前源码桌面程序重建；最终输出为 `src-tauri/target/release/bsaigc_desktop.exe`，文件大小 `33,213,952` 字节，写入时间 `2026-08-02 12:16:12`，SHA-256 为 `CBF37F46EE1FE68B3EA711641847E9B9047F548827E3888804A4F7568A58E230`。
- 对最终 EXE 执行二进制字符串扫描，`legacyTarget`、`?legacy=1`、`BusinessWorkbench`、`DesktopShell`、`BusinessDocumentsCenter`、`返回 1.0 新版`、`旧版入口` 命中均为 `0`；当前桌面二进制不再携带可识别的旧壳入口标记。
- 最终 EXE 隐藏冷启动运行 `8` 秒后进程仍存活且 Windows `Responding=True`，随后测试进程正常停止；这确认当前重建桌面程序至少通过启动级冒烟，不再误启用 `2026-07-30` 的历史 release 二进制。
- 资源竞争期间并行 Vitest 曾在全部断言通过后出现 worker 退出失败；Rust 构建结束后改为单 worker 串行重跑，最终 `28` 个测试文件、`181/181` 测试通过并以退出码 `0` 结束，确认不是以忽略异常的方式收尾。
- 浏览器再次访问 `?legacy=1` 时标题仍为“半山商务工作台 1.0”，登录门禁正常显示且旧版、返回新版、切换新版、新版入口及旧组件名命中均为 `0`。结合已登录态历史资料弹层冒烟结果，产品层只保留 1.0 新壳；历史数据读取、迁移和模板兼容属于底层兼容能力，不构成第二套产品入口。

## 29. 2026-08-02 发布策略收口为单一正式 1.0

- 用户决定取消可独立分发的内部 RC、Windows RC、macOS RC 和跨平台 RC 节点；总计划已改为所有功能、设备、升级回滚、签名、公证、数据完整性和五个真实商务使用门禁全部关闭后，再基于同一干净 Git 提交一次性发布正式 1.0。
- 第 14 天和第 20-21 天产生的安装包、ZIP、GitHub Artifact、R2 对象和 `release/1.0.0` 至 `release/1.3.4` 历史目录全部降级为隔离验收或升级回滚证据；不得创建 GitHub 正式 Release，不得更新 R2 `version.json` 或当前下载入口，也不得只发布已通过的平台。
- 当前工作树已生成发布前源码快照：`1495` 个文件、`44` 个排除项、敏感信息命中 `0`；快照目录为 `.runtime/source-snapshots/bsaigc-desktop-source-business-1.0-precommit-20260802-206bc8360616-20260802T161341Z`，最终快照指纹 `edb7453f07ac6a7493fab161d41ebd95e5a3b348ec3f46017f8158b323afcf8f`，源码 ZIP SHA-256 为 `2e2da514118bd6bf877066bed36079ce0dc5b93a7f24c93df8e866d75ca8ab18`。
- 后续发布顺序固定为：完成本机门禁、第二台 Windows/独立用户、macOS ARM64 与资源验证、Windows/macOS 签名公证、五个真实商务使用、全量回归；随后提交推送最终代码，从同一提交生成双平台产物，先上传不可变版本目录并回读，最后同时创建 GitHub 正式 Release 并更新 R2 当前版本清单。
- 任一门禁失败时整个正式发布批次停止，继续修复和重跑；上一正式版本和当前更新清单保持不变，中间验证产物只保存在受限证据目录。

## 30. 2026-08-02 正式证据链、统一发布回滚与源码快照安全门禁

- 新增正式发布证据聚合验证器与 macOS 证据验证器，并接入 `pnpm release:verify`：正式证据必须绑定同一完整 `40` 位 Git SHA、同一程序版本、Windows/macOS 双平台资产、人工验收和版本说明；当前正反向测试为 macOS `15/15`、正式聚合 `3/3`，跨提交拼接、签名/公证/启动失败、人工验收不完整和非门禁产物均会被拒绝。
- 统一正式发布 workflow 保持唯一人工 `workflow_dispatch` 入口；Windows 构建链只产出 `distributionAllowed=false` 的隔离验证证据，不包含 tag 自动发布、独立 GitHub Release、R2 正式上传或当前版本清单更新旁路。
- 正式发布失败回滚已收口：在调用 `gh release create` 前预置 Release/tag 清理状态；失败时依次恢复 R2 `version.json` 与 `version-mac.json`、删除本批次 immutable prefix、最后删除 GitHub Release/tag，避免 Release 创建过程中只留下 tag 或 R2 半发布状态。工作流 YAML、Bash/PowerShell 块、回滚状态模拟和 `git diff --check` 已通过；本机未配置 `gh` CLI，未触碰真实 GitHub/R2 发布状态。
- 完整本地门禁首次被 Rust 格式检查拦截，已使用项目格式器修复 `video_completion_acceptance_template.rs` 和 `business_workspace_service.rs` 的两处格式漂移，并在 `CARGO_BUILD_JOBS=1` 下重跑 `pnpm release:verify` 通过；前端 `28 files / 181 tests`、协议生成、TypeScript、生产构建、Rust fmt/check/clippy/test、Codex sidecar 签名、macOS/正式证据测试和 `git diff --check` 全部以退出码 `0` 收口。
- 源码快照候选审查发现 `docs/visual` 下 `5` 张未跟踪视觉验收 PNG 包含可识别的客户/项目/合同演示信息，而二进制截图不会被文本敏感扫描覆盖；已将 `docs/visual/` 同时加入 `.gitignore` 和源码快照排除根，防止进入提交与源码 ZIP，不删除本地视觉证据。
- 修正后源码快照 Dry Run 结果为：包含 `1496` 个文件、排除 `45` 项、敏感信息命中 `0`，最终快照指纹 `5095dd5f97be4fbc9a99668cfb65c1ae141cc5a9b52d788c5de359dc9f21fed5`；合成 fixture 未产生误报，百度网盘分享凭据、用户主目录、本地客户素材路径和真实业务材料规则继续生效。
- 当前 Dry Run 和第 29 节预提交 ZIP 都只是脏工作树恢复/审计证据，不是正式发布源码包。工作树尚未形成干净提交，第二台 Windows、不同用户、Windows Authenticode、macOS ARM64/Developer ID/Apple 公证/Gatekeeper/启动烟测及至少 `5` 个真实商务案例仍未关闭，因此本轮正确阻断真实最终源码 ZIP、GitHub Release、R2 immutable 上传和当前版本清单更新。

## 31. 2026-08-02 发布策略自动验证与源码快照纵深加固

- 本轮按唯一执行合同继续推进统一正式 1.0 门禁收口；只读一致性审计确认总计划实际 SHA-256 为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，与执行报告登记值一致。当前阶段不是内部 RC、Windows RC、macOS RC 或跨平台 RC，历史 `windows-rc-*`、候选包和“1.0 RC”名称仅代表隔离测试、升级与回滚证据，不具备分发资格。
- 新增 `scripts/verify-release-workflow-policy.mjs` 及其测试并接入 `pnpm release:verify`；验证器固定检查 Windows、macOS 和统一正式推广三个 workflow，拒绝 push/tag 自动触发、单平台创建 GitHub Release、单平台更新 R2 当前版本清单、缺少精确人工确认、跨提交证据、错误发布顺序和错误回滚顺序。专项测试 `9/9` 通过，真实 workflow 验证结果为 `valid: true`，YAML lint 通过。
- macOS 构建 workflow 和统一正式推广 workflow 已在读取真实证据前执行 macOS evidence、formal evidence 与 release workflow policy 测试；当前专项门禁合计为 macOS `15/15`、正式证据聚合 `3/3`、发布策略 `9/9`、源码快照 `10/10`，共 `37/37` 通过。
- 源码快照安全审计发现并修复三类旁路：排除目录虽然不进入 ZIP，但其已跟踪二进制修改仍可能进入 `git-diff.binary.patch`；小型 DOCX/XLSX/PDF/视频或非可信目录图片可能因二进制内容不接受文本扫描而进入 ZIP；`operator` 或含宽松 synthetic 关键词的真实用户目录可能被误放行。修复后，排除目录同时加入 Git pathspec，业务文档与视频扩展直接阻断，图片只允许位于 `public`、`src/assets` 和 `src-tauri/icons`，用户目录 synthetic 规则只接受明确占位符或 Public/Default 类系统目录。
- 脏工作树 diff 可能包含当前源码已经删除、但仍存在于 HEAD 删除行中的旧路径或凭据字面量；快照脚本现在在保存 `git-diff.binary.patch` 前执行值级脱敏，并在 manifest 记录 `gitDiffRedactions`。新增回归覆盖 tracked `docs/visual` 二进制 diff 排除、业务文档二进制阻断、非可信图片阻断、operator 用户目录阻断和历史 diff 值脱敏；快照测试由 `6/6` 扩展为 `10/10`。
- 为避免安全扫描器把测试目的路径当作可分发字面量，Rust 测试中的 Windows/Unix 用户目录样例改为编译期或运行期片段组合，测试语义保持不变；首次完整门禁因此被 Rust fmt 检查拦截，使用项目格式器修复后重新执行。
- 最新真实仓库源码快照 Dry Run 通过：Git HEAD 为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，包含 `1499` 个文件、排除 `45` 项、当前源码敏感命中 `0`、脏 diff 旧值脱敏 `10` 处，最终快照指纹为 `27bea1731e41fd0bc9b91f8131c36cb023ae9ceac19349aae53a2a35155f8fa9`；本轮只执行 Dry Run，没有生成或提升正式源码 ZIP。
- 最终在 `CARGO_BUILD_JOBS=1` 下重跑 `pnpm release:verify` 通过：业务技能包 `14 skills / 14 tools / 35 files`、前端 `28 files / 181 tests`、协议生成、TypeScript、生产构建、Rust fmt/check/clippy/test、Codex sidecar 签名、macOS/formal/workflow/snapshot 证据测试和 `git diff --check` 全部以退出码 `0` 收口。
- 正式发布继续阻断，且本轮没有执行提交、推送、GitHub Release、R2 immutable 正式上传或 `version.json` / `version-mac.json` 更新。剩余外部门禁仍为：形成干净 Git 基线；第二台不同 MachineGuid 的 Windows；不同用户 SID；Windows Authenticode；macOS ARM64 构建、Developer ID、Apple 公证、Gatekeeper、真实启动与升级；至少 `5` 个真实商务案例；从同一干净 Git SHA 生成双平台最终产物；外部门禁关闭后的最终全量回归。任一门禁失败时整个正式 1.0 批次停止，不得只发布已通过的平台。

## 32. 2026-08-02 全工作流唯一发布者与 R2 并发事务门禁

- 本轮继续以总计划为唯一执行合同，核对总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，Git HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`。多窗口分别审计全 workflow 发布旁路和 R2 current manifest 并发竞争；并行窗口上游连接失败后，主线接管其已落盘改动，完成代码审查、修复、测试和全量集成，没有丢弃或覆盖现有用户变更。
- `scripts/verify-release-workflow-policy.mjs` 已由固定三文件检查扩展为自动枚举 `.github/workflows/*.yml` / `*.yaml`。任何新增 workflow 只要创建或发布 GitHub Release、创建/推送/删除 tag、上传任意 R2/S3 发布对象、写 `version.json` / `version-mac.json` 或设置 `distributionAllowed=true`，都会被门禁拒绝；唯一允许的发布者仍是 `.github/workflows/promote-business-workbench-1.0.yml`。新增动态 workflow 回归后，发布策略专项测试由 `9/9` 扩展为 `12/12`，真实仓库扫描结果为 `valid: true`。
- 审计发现仓库根目录仍跟踪 `_ci_build_windows_yml.txt` 与 `_ci_build_macos_yml.txt` 两个历史直发模板；它们包含 tag 自动触发、构建时写入 R2 凭据和单平台直接覆盖 current manifest 的旧流程，虽非 GitHub Active Workflow，仍会进入源码快照并形成误启用旁路。本轮已物理删除两个无引用模板，正式发布只保留统一推广 workflow。
- 统一推广 workflow 的 R2 发布事务已升级为对象所有权与 ETag CAS：旧 current manifest 使用 `head-object` 固定 ETag 后条件读取；immutable 资产使用 `put-object --if-none-match '*'`，逐对象记录本次创建的 key/ETag 并按 ETag 读取校验，不再允许递归删除整个 prefix；`version.json` 与 `version-mac.json` 分别使用 `If-Match` 或 `If-None-Match` 条件写，并记录本轮写入后的 ETag。
- 失败回滚只恢复或删除仍由本轮 ETag 标识的 current manifest，只删除本轮实际创建且 ETag 未变化的 immutable 对象；如果其他发布者已改写对象，回滚会拒绝破坏其状态并明确报错。GitHub Release/tag 的清理所有权改为 `gh release create` 成功后才置位，避免创建命令失败时误删同名外部状态。新增 `scripts/verify-r2-release-transaction.mjs` 与 `6/6` 回归并接入 `pnpm release:verify`，覆盖不可覆盖上传、current manifest CAS、禁止递归回滚、回滚所有权和 Release 创建时序。
- 本轮专项合计为 macOS evidence `15/15`、正式证据聚合 `3/3`、发布策略 `12/12`、R2 发布事务 `6/6`、源码快照 `10/10`，共 `46/46` 通过。随后在 `CARGO_BUILD_JOBS=1` 下执行完整 `pnpm release:verify`，业务技能包 `14 skills / 14 tools / 35 files`、前端 `28 files / 181 tests`、协议生成、TypeScript、生产构建、Rust fmt/check/clippy/test、Codex sidecar 签名和全部发布门禁均以退出码 `0` 收口；`git diff --check` 通过。
- 最新真实仓库源码快照 Dry Run 继续通过：包含 `1499` 个文件、排除 `45` 项、当前源码敏感命中 `0`、脏 diff 旧值脱敏 `10` 处，最终快照指纹更新为 `38742382167cbfa1d09b15a9d51c884a94c46c872cebf1555cdf4ae2919e6499`；没有生成正式源码 ZIP。
- 正式发布继续阻断，本轮没有提交、推送、创建 tag/GitHub Release、上传 R2 immutable 资产或更新 current manifests。剩余外部门禁仍为：形成干净 Git 基线；第二台不同 MachineGuid 的 Windows；不同用户 SID；Windows Authenticode；macOS ARM64 构建、Developer ID、Apple 公证、Gatekeeper、真实启动与升级；至少 `5` 个真实商务案例；从同一干净 Git SHA 生成双平台最终产物；外部门禁关闭后的最终全量回归。任一门禁失败时整个正式 1.0 批次停止。

## 33. 2026-08-02 本机门禁复核与外部条件冻结

- 多窗口只读复核确认总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，Git HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`；当前工作树共 `186` 项状态变化，未形成可发布的干净基线。
- 在 `CARGO_BUILD_JOBS=1` 下重跑 `pnpm release:verify`，耗时约 `322` 秒并以退出码 `0` 收口；前端、Rust、Codex sidecar、macOS/formal evidence、workflow policy、R2 transaction、source snapshot 和 `git diff --check` 全部通过。本轮未产生新的 tracked diff，可安全重复执行。
- 发布策略专项独立复核为 `valid: true`，扫描到的 workflow 仍只有 `build-macos.yml`、`build-windows.yml` 和唯一正式发布者 `promote-business-workbench-1.0.yml`；R2 发布事务回归保持 `6/6` 通过。没有提交、推送、创建 tag/GitHub Release、上传 R2 或更新 current manifests。
- 当前仍不能生成正式 `final-gates.json` 或关闭正式发布门禁：第二台不同 MachineGuid 的 Windows、不同用户 SID、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper/真实启动升级、至少 `5` 个真实商务案例，以及同一干净 Git SHA 的双平台最终构建均需要外部设备、凭据或真实业务输入；缺少这些输入时继续生成正式证据会制造虚假验收，故保持阻断。

## 34. 2026-08-02 新 1.0 响应式真实浏览器冒烟与共享案例入口修复

- 本轮以新 1.0 产品壳真实浏览器冒烟为最高价值纵切；桌面 `1280×720`、平板 `768×800`、手机 `390×844` 均加载标题“半山商务工作台 1.0”，页面无横向溢出，控制台无 warning/error，页面文本和当前 import graph 均未发现可达旧版入口。桌面入口保持 `x=1158,y=608,w=104,h=40`，未受移动端修复影响。
- 冒烟发现共享案例浮动入口在窄屏会遮挡任务输入、发送按钮或快捷任务条：修复前 `768×800` 入口 `y=688` 与输入区重叠，`390×844` 入口 `y=736` 与发送区重叠。首轮固定抬升到 `183px` 后静态几何通过，但多窗口 CSS 审计继续识别出移动抽屉层级冲突、safe-area 覆盖不完整和输入区动态增高时固定偏移失效三项风险。
- `SharedCaseCenter` 现通过 `ResizeObserver`、窗口 resize 和 `matchMedia('(max-width: 900px)')` 实时测量现有 `.bw-composer-zone`，只在窄屏更新 `--bw-shared-case-launcher-bottom`，不创建第二套状态引擎或业务服务；入口始终与动态输入区保持 `8px` 间距。手机输入区从 `175px` 增长到 `260px` 时，入口偏移由 `183px` 自动更新为 `268px`，入口从 `y=619` 自动移动到 `y=534`，未再覆盖输入区。
- 移动端入口层级降到 `z-index: 2`，低于遮罩 `3` 和左右抽屉 `6`；真实打开项目抽屉后，入口、遮罩、抽屉层级分别为 `2/3/6`。共享案例面板、抽屉和输入区补齐 safe-area；`390×844` 下共享案例 backdrop 四边 padding 为 `8px`，面板为 `x=8,y=8,w=374,h=828`，无横向溢出。
- 修复后真实几何为：`768×800` 入口 `y=595,h=40`，输入区顶边 `y=643`，保留 `8px` 间距；`390×844` 入口 `y=619,h=42`，输入区顶边 `y=669`，同样保留 `8px` 间距。桌面恢复默认 viewport 后入口内联移动偏移为空，继续使用原桌面定位与 `z-index: 7`。
- 专项测试 `SharedCaseCenter.test.tsx` 与 `ChatWorkspace.test.tsx` 为 `10/10` 通过；修改后全量前端测试为 `28 files / 181 tests` 通过，`pnpm build`、TypeScript、Vite production build 和 `git diff --check` 均以退出码 `0` 收口。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树仍为 `186` 项状态变化。
- 正式 1.0 发布状态不变：当前工作树的新壳、旧 UI 删除和本轮修复尚未形成干净 Git 基线；第二台 Windows、不同 SID、Windows 签名、macOS ARM64/Developer ID/公证/Gatekeeper、至少五个真实商务案例及同一干净 SHA 双平台构建仍未关闭。本轮未提交、推送、创建 Release、上传 R2 或更新 current manifests。

## 35. 2026-08-03 验收 XLSX 输出格式闭环与真实入口链加固

- 本轮按总计划第 18 节重新映射验收项与现有强证据，并由两个只读窗口分别审计报价/资料库/年框和验收/document_engine/人工确认链。审计确认年框结算已有代码级强覆盖，但发现新 1.0 的验收文档生成把“报价以外全部格式”硬编码为 `docx`；合同结算验收输出规格实际为 `xlsx`，Rust 服务端会严格拒绝格式不匹配，因此正常新界面无法完成计划要求的 `4 DOCX + 1 XLSX` 真实五文件链。
- `BusinessV1App` 新增唯一格式解析函数 `businessDocumentOutputFormat`：报价继续确定为 `xlsx`，普通合同/请款继续为 `docx`，验收文档则沿现有 `BusinessWorkspaceRecord.acceptanceBatches[].outputSpecs[].format` 解析；已绑定批次和输出规格但找不到对应规格时失败关闭，禁止以猜测格式生成正式文件。该修复只复用现有 SQLite/business workspace/document_engine/Client SDK，不创建第二套数据库、任务引擎或文档引擎。
- 新增三组回归覆盖：验收合同结算规格正确选择 `xlsx`；已绑定但缺失输出规格时明确阻断；报价保持 `xlsx`、普通合同保持 `docx`。专项 `BusinessV1App.test.tsx` 与 `RootApp.test.tsx` 共 `9/9` 通过。
- 入口测试同时去除对 `BusinessV1App` 的假壳 mock，`RootApp.test.tsx` 现在直接渲染真实 `RootApp → BusinessV1App → BusinessWorkspaceShell` 链；真实业务壳增加稳定的 `data-product-shell="business-v1"` 标识，并验证“半山商务工作台”“选择项目后开始任务”及无 `legacy` 文本，关闭了此前只能证明 mock 组件而不能证明真实入口的测试缺口。
- 修改后全量前端测试为 `28 files / 184 tests` 通过，`pnpm build`、TypeScript、Vite production build 和 `git diff --check` 均以退出码 `0` 收口。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树仍为 `186` 项状态变化。
- 下一最高价值本机纵切已锁定为“新壳独立报价可达闭环”；并行窗口正在限定写集内实现报价中心组件和测试，主线后续负责接入现有 business workspace/profile/真实 XLSX 生成与人工确认链。正式发布门禁保持不变，本轮未提交、推送、创建 Release、上传 R2 或更新 current manifests。

## 36. 2026-08-03 独立报价编辑、人工审批与 XLSX 闭环

- 新 1.0 壳已增加独立报价中心，报价任务从现有快捷入口直接进入，不扩建或恢复旧业务界面。中心可编辑服务名称、说明、数量、单位、含税单价、行税率、项目优惠、默认税率和计税方式，并显示报价版本、审批状态和输出格式。
- 报价预览复用既有 `createQuotationDraft` 领域计算；正式保存和生成继续复用同一 `business_workspace_service`、SQLite、Vault、`document_engine` 与 Client SDK。命令链为 `updateBusinessProfile → createBusinessDocument(quote) → draft/inReview/approved → generateBusinessDocument(xlsx)`，每一步严格使用上一条返回的 `businessWorkspace.revision`，避免 CAS revision 冲突。
- 报价仍执行必须人工确认的业务边界：首次提交创建报价文档并推进到 `inReview`，第二次明确人工操作推进到 `approved`，只有 `approved` 才允许生成真实 XLSX；已生成成果可从 Vault 打开，也可创建新版本继续报价。白鹅潭基准保持 `21,200 × 4 = 84,800`、项目优惠 `4,900`、最终报价 `79,900`，数量和单价分别保存，不允许通过改动单价凑总价。
- 真实浏览器桌面冒烟发现兼容缺陷：预览中的旧业务资料缺少后续新增的 `projectDiscountCents` 和 `taxMode` 字段时，报价中心把优惠渲染为空并误报“项目优惠不能为负数”，导致保存与审批按钮全部禁用。现已在 `quotationCenterInput` 兼容旧资料：缺失优惠归一为 `0`，缺失计税方式归一为 `taxInclusive`，缺失行税率回退到默认税率；新增回归证明旧资料可正常进入报价链。
- 修复后完成真实浏览器桌面与 `390×844` 移动断点复验：报价金额、版本、状态、输出格式和按钮门禁正确；`generated` 状态下新版本按钮可用、`生成 XLSX` 保持禁用；移动端 `body/html scrollWidth = clientWidth = 390`，无横向溢出，固定操作区可见。视觉证据保存为 `release-evidence/browser-qa/quotation-center-desktop-20260803.png` 和 `release-evidence/browser-qa/quotation-center-mobile-390x844-20260803.png`。
- Rust 独立验收链同步关闭计划第 18 节第 3 项缺口：外部已审合同晋升不再错误要求本系统 Quote 或报价确认，普通合同创建的报价门禁保持不变。新增测试从外部合同和审查报告进入 Vault 开始，完整覆盖晋升有效合同、明确无 Quote、创建验收批次、绑定素材、准备/审批文档、复用现有 `document_engine` 生成 DOCX 和验收文件生效；`CARGO_BUILD_JOBS=1 cargo test ... independent_acceptance_promotes_external_reviewed_contract_without_system_quote` 为 `1 passed / 0 failed / 831 filtered out`。
- 本轮最终验证：报价与应用链专项 `17/17`，全量前端 `29 files / 193 tests`，`pnpm check`、`pnpm build`、`cargo fmt --check` 和 `git diff --check` 全部通过；`git diff --check` 仅输出仓库既有 CRLF/LF 提示。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树仍为 `186` 项状态变化。
- 正式 1.0 发布门禁不变：当前仅关闭独立报价和“无本系统报价的独立验收”代码/本机验证纵切，尚未形成干净 Git 基线，也未关闭第二台 Windows、不同 SID、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、升级回滚、五个真实商务案例及同一干净 SHA 双平台构建门禁。本轮未提交、推送、创建 GitHub Release、上传 R2 或更新 current manifests。

## 37. 2026-08-03 独立验收中心五文档全链路与浏览器状态隔离修复

- 新 1.0 壳的独立验收中心已完成可运行闭环：从现有项目工作区创建验收批次，准备固定五份验收草稿，执行六类内容与素材组数缺失门禁，绑定素材后逐份提交复核、人工批准并生成成果。实现继续复用既有 `business_workspace_service`、SQLite、Vault、`document_engine`、R2/OCR 资产能力与 Client SDK，没有创建第二套数据库、任务引擎、Agent Runtime 或文档引擎，也没有扩建旧业务界面。
- 白鹅潭式浏览器冒烟实际绑定九组素材：视频成片四组，脚本、截图、花絮、发布数据和验收证明各一组。缺素材时五个“提交复核”入口全部保持禁用；素材齐备后五份文档均完成 `draft → inReview → approved → generated`，最终形成 `1 XLSX + 4 DOCX`，并显示五个可打开的成果入口，保留人工审批和正式生成边界。
- 真实浏览器首次创建批次时发现弹窗空白缺陷：预览 mock 直接返回共享可变对象，缺少真实 Tauri IPC 的序列化隔离；命令处理中的原地数组和 revision 更新同步污染了 Client Projection 持有的旧对象，使 `upsert` 误判 revision 未增长且 React 没有获得新数组引用。`preview/mock-tauri.ts` 的 `invoke` 现对命令结果执行 `structuredClone`，模拟真实 IPC 的值传递语义，创建批次后可立即显示当前批次、六类要求、缺失门禁和草稿操作。
- 修复后完成桌面和 `390×844` 移动断点人工冒烟。移动端 `body/html clientWidth = scrollWidth = 390`，`innerWidth = 390`、`innerHeight = 844`，`dialogCount = 1`、`generatedButtons = 5`，无横向溢出。视觉证据保存为 `release-evidence/browser-qa/acceptance-center-desktop-20260803.png` 与 `release-evidence/browser-qa/acceptance-center-mobile-390x844-20260803.png`。
- 专项前端验证 `AcceptanceCenter.test.tsx` 与 `BusinessV1App.test.tsx` 为 `16/16` 通过；最终全量前端为 `30 files / 198 tests` 通过，`pnpm check`、`pnpm build` 和 `git diff --check` 均以退出码 `0` 收口。生产构建转换 `1635` 个模块，主 JS 为 `470.05 kB`、gzip 后 `133.44 kB`，无 bundle-size 或编译警告；仅保留既有换行符提示和非失败的慢用例提示。
- Rust 最小回归继续统一使用 `CARGO_BUILD_JOBS=1`，独立验收无本系统 Quote、素材缺失门禁及五文档链、真实验收 XLSX 包三项均通过，合计 `3 passed / 0 failed`。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树仍为 `186` 项状态变化。
- 正式发布门禁仍未关闭：第二台 Windows 与不同 SID、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、升级回滚、五个真实商务案例及同一干净 SHA 双平台构建尚无完整证据。本轮未提交、推送、创建 GitHub Release、上传 R2 或更新任何 `current` manifest；下一最高价值纵切锁定为验收中心失败恢复与重复提交门禁，优先验证快速双击、失败后输入保留、重复准备固定五份以及素材类型/归属服务端校验。

## 38. 2026-08-03 验收中心重复提交、素材类型门禁与结构化错误闭环

- 验收中心所有会产生持久化副作用的入口统一增加同步 pending 锁，覆盖创建批次、绑定素材、准备五份草稿、提交复核、批准、生成和打开成果；快速双击创建只形成一个批次，快速双击准备仍固定生成五份草稿，失败后表单输入保留并在局部展示错误与重试入口。
- Rust 服务端验收素材门禁现同时校验项目归属、资产状态和要求类型；不匹配时返回 `BUSINESS_ACCEPTANCE_ASSET_KIND_MISMATCH`，且命令保持原子失败，不写入错误绑定。测试夹具按要求类型生成 MP4、PNG、PDF 或 BIN，修复旧夹具一律以 `.bin/other` 导入导致的全量回归冲突。
- 前端错误归一化现支持真实 Tauri Host 返回的普通结构化对象 `{ code, message, retryable }`，不再只识别 JavaScript `Error` 实例；新增结构化 HostError 回归，确保真实 Host 错误码与消息可直接展示。
- 真实浏览器复核确认：快速双击创建仅生成一个批次，快速双击准备仍只有固定五份文档，缺少素材时五个“提交复核”入口全部禁用，素材选择器按要求类型过滤，控制台无 error/warn。视觉证据保存为 `release-evidence/browser-qa/acceptance-center-reliability-desktop-20260803.png`。
- 前端专项回归为 `2 files / 23 tests`，全量前端为 `30 files / 205 tests`；`pnpm check`、`pnpm build`、`cargo fmt --check`、`cargo check --lib` 和 `git diff --check` 均通过。生产构建转换 `1635` 个模块，主 JS 为 `472.16 kB`、gzip 后 `134.18 kB`。
- Rust 全量库测试继续使用 `CARGO_BUILD_JOBS=1`，结果为 `821 passed / 13 ignored / 0 failed`，共 `834` 项。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树未提交。
- 正式发布门禁继续保持阻断：不得生成正式 gate manifest，不得提交、推送、创建 GitHub Release、上传正式 R2 或更新当前版本清单。下一纵切为 Preview Mock 可控首次失败/延迟、与 Rust 一致的素材校验，以及浏览器失败恢复证据。

## 39. 2026-08-03 Preview 故障恢复注入、八类素材矩阵与全量回归

- Preview Mock 新增可控故障参数：`previewDelayMs=0..10000` 只延迟业务命令，`previewFailOnce=<完整 commandType>` 对指定业务命令首次返回可重试的结构化 `PREVIEW_INJECTED_FAILURE`，后续同一命令自动恢复；不复制第二套业务状态机。实现位于 `preview/mock-tauri.ts`。
- Preview Mock 的验收素材门禁已与 Rust 对齐：校验 workspace、batch、requirement、material kind、资产存在性、`ready` 状态、项目归属以及八类素材到 `image/video/document/other` 的映射；类型不匹配返回 `BUSINESS_ACCEPTANCE_ASSET_KIND_MISMATCH`。
- 新增八类素材过滤矩阵回归，覆盖 `script`、`video`、`screenshot`、`behindTheScenes`、`publishingData`、`invoice`、`proof`、`other`，同时验证兼容资产出现、不兼容资产排除和规范化别名；验收中心专项达到 `18/18`。
- 真实浏览器使用 `previewDelayMs=800&previewFailOnce=businessWorkspace.createAcceptanceBatch` 完成首次失败与恢复：失败时批次尚未落库、输入“失败恢复冒烟批次”保留、局部错误显示且按钮可重试；重试后只生成一个批次并进入“素材收集中”。证据保存为 `release-evidence/browser-qa/acceptance-center-failure-retained-input-desktop-20260803.png` 与 `release-evidence/browser-qa/acceptance-center-failure-recovered-desktop-20260803.png`，已人工检查布局无异常。
- 前端全量回归为 `30 files / 212 tests`，`pnpm check` 和 `pnpm build` 均通过；生产构建转换 `1635` 个模块，主 JS 为 `472.16 kB`、gzip 后 `134.18 kB`。Rust 全量库回归继续使用 `CARGO_BUILD_JOBS=1`，结果为 `821 passed / 13 ignored / 0 failed`。
- 本轮未提交、推送、创建 GitHub Release、上传正式 R2 或更新 `current` manifest。正式发布仍被第二台 Windows/独立 SID、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、真实升级回滚、五个真实商务案例和同一干净 SHA 双平台构建门禁阻断。

## 40. 2026-08-03 正式发布顺序、双平台证据真实性与第二机门禁加固

- 发现并修复正式发布顺序与总计划不一致的问题：原流程在公开 GitHub 正式 Release 之前更新 R2 `version.json` / `version-mac.json`。现统一为“不可变资产上传与回读 -> 创建 draft Release -> 公开正式 Release -> 最后 CAS 更新并回读 R2 当前版本清单”；`verify-release-workflow-policy.mjs` 新增强制顺序检查和反向回归，防止当前版本清单再次提前发布。
- macOS 证据校验器新增真实 UTC 时间、GitHub workflow run identity、非空证据文件、DMG 文件名与版本/`aarch64-apple-darwin`/12 位 commit 绑定、路径穿越及符号链接拒绝；正式证据校验器要求人工验收引用真实非空文件并记录 SHA-256，Release Notes 必须包含唯一独立的 `releaseStatus: formal-1.0-approved` 字段，同时保留签名、公证、Stapler、Gatekeeper、sidecar 和启动烟测的原生门禁 lineage，且输出不再泄漏本机绝对路径。
- Windows 第二机 bundle 与 runner 加固：非 dry-run 不允许关闭不同机器和不同用户 SID 门禁，连续冷启动不得低于 `20` 次，上一版本必须低于当前版本；安装包、manifest 与 `SHA256SUMS.txt` 强绑定，并拒绝重复 checksum、路径逃逸、junction/reparse point 和 Windows 保留设备名；验收步骤要求恰好一条，bundle `-DryRun` 可重复执行。数据迁移回滚脚本完成 Parser、dry-run 与隔离 fixture/upgrade/backup/rollback/uninstall 验证，未触碰真实 AppData。
- 发布门禁专项总回归 `55/55` 通过：macOS evidence `18/18`、formal evidence `8/8`、workflow policy `13/13`、R2 transaction `6/6`、source snapshot `10/10`；三份 Windows PowerShell 脚本 Parser 检查和三个 Node 校验器 `node --check` 通过，`git diff --check` 通过，仅保留既存换行符提示。本轮没有 UI 变更，沿用第 39 节已完成的真实浏览器视觉证据。
- 对当前 `release-evidence` 直接运行正式校验仍按预期失败：缺少 `release-evidence/windows` 和 `release-evidence/macos` 的真实双平台门禁目录，未生成任何“通过”的正式 gate manifest。总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，HEAD 仍为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树仍为 `186` 项状态变化且未提交。
- 正式 1.0 继续硬阻断：真实第二台 Windows/不同 SID/20 次冷启动、真实跨版本升级和故障注入回滚、Windows Authenticode、macOS ARM64/Developer ID/公证/Stapler/Gatekeeper/启动烟测、五个真实商务案例以及同一干净 Git SHA 的双平台产物仍未闭环。本轮未提交、推送、创建 GitHub Release、上传正式 R2 或更新任何 `current` manifest。

## 41. 2026-08-03 Windows 第二机证据闭环与自动化回归

- Windows 第二机 runner 进一步收紧正式发布资格：在调用验收引擎前即拒绝既有 evidence 目录冲突，避免覆盖或混合历史证据；`Upgrade`、`Rollback` 与所有 `DryRun` 模式都明确输出 `releaseGateEligible=false`，只有 `Both` 模式同时满足不同 MachineGuid、不同用户 SID、连续至少 `20` 次冷启动、真实升级、故障注入回滚及升级/回滚两份 backup manifest 全闭环时，才允许输出 `releaseGateEligible=true`。
- 升级和回滚阶段生成的 `backup-manifest.json` 现分别固化为 `upgrade-backup-manifest.json` 与 `rollback-backup-manifest.json`，runner 会重新计算 SHA-256、写入最终 evidence manifest，并将两份文件纳入 `SHA256SUMS.txt`。数据迁移回滚脚本同步生成真实 `rollback/backup-manifest.json`，记录并回读校验 SHA-256，回滚严格依据该 manifest 恢复；`DryRun` 也补齐 backup 计划步骤，但不会伪造任何可用于正式门禁的证据。
- 新增三组隔离自动化回归并接入 `pnpm release:verify`：第二机 bundle 测试 `5/5`，覆盖真实未签名 PE fixture、可重复 DryRun、checksum 路径逃逸、重复 checksum 与 junction 拒绝；第二机 runner 测试 `6/6`，覆盖三种 DryRun 均不具发布资格、`Both` 证据闭环、backup manifest SHA、evidence 冲突前置拒绝、冷启动/保留设备名门禁及源码执行顺序；数据迁移回滚测试 `8/8`，覆盖 Parser、DryRun、隔离 fixture/upgrade/backup/rollback/uninstall、默认清理、AppData 隔离、manifest SHA、路径逃逸与 junction。
- 本轮新 Windows 专项共 `19/19` 通过；既有发布门禁专项 `55/55` 通过，合计发布门禁回归 `74/74`。前端全量回归继续为 `30 files / 212 tests`，`pnpm check` 通过；三份 PowerShell 脚本 Parser、三份新增 `.test.mjs` 的 `node --check` 与 `git diff --check` 均通过，后者仅保留仓库既有换行符提示。本轮未改 Rust，沿用第 39 轮 `CARGO_BUILD_JOBS=1` 全量结果 `821 passed / 13 ignored / 0 failed`。
- 所有新增测试均在隔离临时目录执行，未触碰真实 AppData、正式安装状态或正式 release evidence。手工隔离迁移验收成功，证据目录为 `.runtime/data-migration-rollback/round41-8bac7f3c50c7`，其 manifest SHA-256 为 `4835872f0432186526d4441c466076ce8ba9a280eb3a50b290e0335a9bdf448c`；该目录保持忽略状态，不进入正式发布证据。
- 总计划 SHA-256 仍为 `2C62D3F2E18D696510FE8A5FFF0A65F8B12F02F25281CB4267CDF85D2E6ABC0A`，分支 / HEAD 仍为 `main` / `206bc8360616a9b87b91dac654d5ada4c361ecfb`。正式 gate manifest 未生成；当前仍缺少真实 `release-evidence/windows` 与 `release-evidence/macos` 闭环，不得把本机 fixture 或 DryRun 结果冒充发布证据。
- 正式 1.0 的硬阻断不变：真实第二台 Windows、不同 MachineGuid/SID、连续 `20` 次冷启动、真实跨版本升级与故障注入回滚、Windows Authenticode、macOS ARM64/Developer ID/公证/Stapler/Gatekeeper/真实启动烟测、五个真实商务案例，以及同一干净 Git SHA 的 Windows/macOS 最终产物。本轮未提交、推送、创建 GitHub Release、上传正式 R2 或更新任何 `current` manifest。

## 42. 2026-08-03 本机人工冒烟、R2 状态接线与全量门禁收口

- 真实浏览器重新登录 `http://127.0.0.1:1422/preview/index.html`，验证新 1.0 主工作区、项目/对话、快捷任务和独立验收入口可用；登录使用隔离 mock 账号，不触碰正式账号或远端数据。
- 独立验收中心创建隔离批次“本机冒烟验收”，初始正确显示 `6 项阻塞`、六类素材需求和审批/生成阻断。随后绑定 4 组视频成片、脚本、拍摄花絮、发布数据、验收证明，保留视频截图缺口；页面正确收敛为 `1 项阻塞`，并继续保持审批与正式生成不可用。该结果验证了 groupKey 去重、人工确认、素材类型选择、缺失重算和阻断策略。
- R2 设置状态已接入真实 `R2RuntimeStatus`；预览 mock 改用安全 `bsaigc-storage://` capability，页面正确显示 `Cloudflare R2 / 已就绪 / 1 项等待异步备份`，不再出现 unsafe storage root capability 错误。SQLite queued 数量仍保留展示。
- 前端全量门禁：`pnpm test` 为 `30/30` 文件、`212/212` 测试通过；`pnpm check` 通过；`pnpm build` 通过，主 JS `472.16 kB`、gzip `134.18 kB`。
- Rust 全量门禁使用 `CARGO_BUILD_JOBS=1`：`823 passed / 13 ignored / 0 failed`；workspace/all-targets 构建与测试通过。
- 浏览器期间观察到一次浏览器基础设施 Statsig 上报超时；未观察到业务页面错误、阻塞异常或 unsafe storage capability。窄屏、完整验收草稿生成和最终 `release:verify` 仍需本轮后续收口。
- 本机安装包继续作为受限测试产物：`src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe`。未提交、未推送、未创建 GitHub Release、未上传正式 R2、未更新 `current` manifest。
- 正式 1.0 硬门禁不变：真实第二台 Windows/不同 MachineGuid 与 SID、macOS ARM64/签名/公证/Gatekeeper、Windows Authenticode、五个真实商务案例，以及同一干净 Git SHA 的双平台最终产物仍缺失；本机结果不得替代这些外部门禁。

## 43. 2026-08-03 拆分发布门禁复核与浏览器烟测续跑

- 使用 `CARGO_BUILD_JOBS=1` 拆分复跑聚合发布校验，避免把此前超时的 `pnpm release:verify` 误报为通过。主线与只读并行窗口均得到一致结果：`rust:fmt:check`、`rust:check`、`rust:clippy`、`codex:sidecar:verify`、macOS 发布证据测试、正式发布证据测试、发布工作流策略测试、R2 发布事务测试、源码快照测试、Windows 第二机验收脚本测试、数据迁移/回滚验收脚本测试以及 `git diff --check` 共 `12/12` 项退出码均为 `0`。
- 拆分测试明细：macOS 发布证据 `18/18`、正式发布证据 `8/8`、发布工作流策略 `13/13`、R2 发布事务 `6/6`、源码快照 `10/10`、Windows 第二机验收脚本 `11/11`、数据迁移/回滚验收脚本 `8/8`。仅观察到现有文件 LF/CRLF 提示，不构成失败。
- 浏览器重新接管现有预览页并使用隔离 mock 账号登录；新 1.0 主工作区、项目/对话、业务文档卡片、快捷任务、联网来源提示和管理员入口继续正常。独立验收中心成功创建隔离批次“本机冒烟验收-最终”，初始状态正确显示 `6 项阻塞`，六类素材需求、审批/生成阻断和“允许提前准备草稿”的提示均符合合同。
- 本机 Windows 受限测试安装包仍为 `src-tauri/target/release/bundle/nsis/半山商务工作台_1.3.4_x64-setup.exe`，大小 `132107499` 字节，SHA-256 为 `A8874B5E4FE9B335434FDFA37AD3AE4000AE47FA6D57711BA36B8CBBB7E45342`。
- 当前 Git HEAD 为 `206bc8360616a9b87b91dac654d5ada4c361ecfb`，工作树包含本轮大量未提交改动；未提交、未推送、未创建 GitHub Release、未上传正式 R2、未更新 `current` manifest。正式 1.0 仍必须等待真实第二台 Windows、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、五个真实商务案例，以及同一干净 Git SHA 的双平台最终产物。
- 浏览器最终剩余项是点击“准备验收草稿”核对五份草稿状态、补一轮窄屏视觉截图并清理测试标签；在这些证据写入报告前，本机门禁仍保持“收口中”，不得标记正式 1.0 完成。
## 44. 2026-08-04 本机 Release 窗口视觉验收与最终收口

- 使用 `src-tauri/target/release/bsaigc_desktop.exe`，通过隔离的 `USERPROFILE`、`APPDATA`、`LOCALAPPDATA`、`TEMP`、`TMP` 与 `CODEX_HOME` 启动真实 Windows Release，不触碰当前用户数据。
- 默认窗口实测保持运行，窗口客户区对应约 `1440x920`；首启界面正确显示管理员初始化、用户名/密码/确认密码、记住账号密码、R2 配置降级提示和创建管理员入口。截图证据：`.runtime/desktop-visual/round44-default/release-window-default.png`。
- 按产品配置的最小窗口 `1120x720` 实测保持运行，登录卡片、按钮、提示和输入区域均完整显示，无横向裁切；截图证据：`.runtime/desktop-visual/round44-min/release-window-min-1120x720.png`。
- 另行强制压缩到 `760x720` 会得到空白 WebView；该尺寸低于 `src-tauri/tauri.conf.json` 声明的 `minWidth=1120`，不是用户可达窗口状态，不纳入产品缺陷结论。浏览器预览已完成 `1280x720` 的新 1.0 业务界面和验收中心视觉/交互烟测。
- 启动日志确认技能包加载成功；R2 缺少本机注入的访问密钥时仅进入明确的降级状态，不阻断本地首启和登录界面。未注入正式 R2 密钥，未上传正式 R2。
- 本节结论：本机 Release 启动、首启可视化、最小允许窗口和业务预览烟测均有证据；本机目标仍不等同于跨平台正式 1.0，正式发布门禁继续保持关闭。

## 45. 2026-08-05 品牌统一、设置页烟测与 Windows Release 重建

- 按品牌整改要求复核新 1.0 设置中心：标题、Logo、桌面图标和应用元数据统一为“华邦互娱商务系统”；设置页正文/按钮字号已提升，主操作高亮统一使用 Logo 橙色渐变，不再混用蓝色或绿色高亮；对话框内未发现“粘贴截图”按钮。
- 在 `1280×720` 浏览器预览中人工复核 AI 服务设置页：设置对话框边界为 `x=150, y=18, w=980, h=700, bottom=718`；“拉取模型”和“保存”按钮均位于 `y=663, bottom=697.5`，未越出 `720` 视口；内容区 `clientHeight=650`、`scrollHeight=662`，只保留内部滚动，不遮挡底部操作区。证据截图：`.runtime/ui-brand-audit/settings-ai-1280x720-after-fix.png`；独立复核截图：`.runtime/ui-brand-audit/settings-ai-services-1280x720-independent-20260805.png`、`.runtime/ui-brand-audit/settings-account-1280x720-independent-20260805.png`。
- 本轮修复紧凑窗口规则：`src/components/SettingsCenter.css` 为桌面窄高度窗口补充 `max-height:760px` 适配，压缩设置容器、编辑卡片、字段间距和底部操作区，根治 `1280×720` 下按钮越界/遮挡；同时将账号/用户表单残留的 `13px` 输入和操作文字统一提升为 `15px`。
- 定向前端回归：`src/components/SettingsCenter.test.tsx` `8/8`、`src/business-v1/ui/ChatWorkspace.test.tsx` `4/4`，合计 `12/12` 通过；完整 `pnpm desktop:build` 通过，Rust 构建统一使用 `CARGO_BUILD_JOBS=1`，并完成 `release:verify`、TypeScript、Vite、Rust fmt/check/clippy/test 及 NSIS 构建。
- 最新本机受限 Windows 测试包：`src-tauri/target/release/bundle/nsis/华邦互娱商务系统_1.3.4_x64-setup.exe`，`132279377` 字节，SHA-256 `48CB7FAAD62E3B9EAC7D1DAC1B7133F819E34BED3F808F2B6A21C4507D414A01`；应用 EXE 为 `33268736` 字节，SHA-256 `FDAFFAFD580A3ED8B632005C6368D314BE6DC4D32288BBB210782C303959E698`。
- Windows 版本元数据已核验：`ProductName=华邦互娱商务系统`、`FileDescription=华邦互娱商务系统`、`CompanyName=华邦互娱`、`FileVersion=1.3.4`；EXE 与安装包 `Authenticode=NotSigned`，因此只能用于本机受限验收，不能作为正式发行包。
- 正式 1.0 门禁继续关闭：第二台不同 `MachineGuid/SID` 的 Windows、Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper/真实启动、至少五个真实商务案例、同一干净 Git SHA 的双平台最终产物仍未齐备；本轮不提交、不推送、不创建 GitHub Release、不上传正式 R2、不更新 `current` manifest。

## 46. 2026-08-05 发布门禁复跑与合同审查纵切闭环

- 在当前工作树、`HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb` 和 `CARGO_BUILD_JOBS=1` 下重新执行 `pnpm release:verify`，前端、TypeScript、Vite、Rust fmt/check/clippy/test、协议/技能生成与发布专项测试全部通过；命令退出码为 `0`。
- 发布专项结果保持通过：macOS 证据规则、正式发布证据、发布工作流策略、R2 发布事务、source snapshot、Windows secondary-machine bundle/runner、数据迁移回滚验收均通过；其中本轮数据迁移回滚专项为 `8/8`，Windows 专项与既有发布门禁未发现回归。
- 运行 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-contract-review-e2e.ps1 -RunId 20260805-closure`，结果为 `Contract review E2E acceptance passed`；证据目录：`.runtime/contract-review-e2e/20260805-closure/`。
- 合同审查闭环 `8/8` 通过：静态 Tauri/Client SDK/NSIS 接口、确定性 DOCX fixture 与 SHA-256、Local Vault 导入、文档提取、规则审查、缺少 Agent 凭据降级、人工 Finding 决策、HTML/DOCX 报告 Artifact、SQLite 重启恢复、命令幂等、备份队列失败不阻断 Completed、R2 传输失败不破坏本地权威数据。
- 本轮未发现可由本机补齐的签名或跨平台输入：Windows 证书存储无可用签名证书，`signtool`/`xcrun`/`codesign`/`notarytool`/`stapler`/`gh` 均不可用；因此正式 1.0 发布门禁继续关闭，不把本机受限包或 fixture/DryRun 结果冒充外部门禁证据。


## 47. 2026-08-05 主工作区字号收口、品牌包重建与冷启动烟测

- 根据本轮视觉复核结果，继续收口 `src/business-v1/ui/business-workspace.css`：主工作区基准字号由 `14px` 提升为 `15px`；主操作按钮、项目对话框标题/字段、项目搜索、导航项目文本、空状态说明和消息头部字号提升 `1px`，保留危险色语义和现有 Logo 橙色高亮体系，不引入第二套主题。
- 在临时清空 R2 凭据的受控构建链中重新执行 `scripts/invoke-windows-rc-local-build.ps1`。Rust 构建统一使用 `CARGO_BUILD_JOBS=1`；前端、TypeScript、协议、Rust fmt/check/clippy/test、发布专项测试和 `git diff --check` 全部通过，NSIS 重新生成完成。
- 最新本机受限 Windows 测试包：`src-tauri/target/release/bundle/nsis/华邦互娱商务系统_1.3.4_x64-setup.exe`，大小 `132178831` 字节，SHA-256 `6D1DB90DB5CBB677F2B2DC98A75FB93B0D4E9EBC23E135B3F161FEC022216A13`；桌面 EXE 大小 `33268736` 字节，SHA-256 `B7978B4E7CFC1EAF5CC9CF6DB95612F5DF21B27093386550D036EDA82E2830DA`。
- 新 EXE 冷启动隐藏运行 `8` 秒后进程仍存活且 `Responding=True`，随后正常停止；二进制字符串扫描中 `legacyTarget`、`?legacy=1`、`BusinessWorkbench`、`DesktopShell`、`BusinessDocumentsCenter`、`返回 1.0 新版`、`旧版入口` 命中均为 `0`，品牌名“华邦互娱商务系统”命中 `11` 次。
- 浏览器预览已在 `1280×720` 登录后进入主工作区，确认项目侧栏、对话区、报价/验收快捷入口、任务输入区和设置入口均可见；登录使用预览 mock，不代表真实账号、第二台 Windows 或跨平台验收。
- 当前包 `Authenticode=NotSigned`，只能作为本机受限验收证据；第二台不同 `MachineGuid/SID` 的 Windows、Windows 签名、macOS ARM64/Developer ID/公证/Gatekeeper、五个真实商务案例和同一干净 Git SHA 双平台产物仍未完成。因此本轮不提交、不推送、不创建 GitHub Release、不上传正式 R2、不更新 `current` manifest`。

## 48. 2026-08-05 隔离 Windows 安装、升级、回滚边界与卸载实测

- 使用最新本机受限包 `src-tauri/target/release/bundle/nsis/华邦互娱商务系统_1.3.4_x64-setup.exe` 执行真实 NSIS 验收：`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/invoke-nsis-release-acceptance.ps1 -InstallerPath <installer> -Version 1.3.4 -RunId 20260805-local-real -ColdStartCount 2 -StartupObservationSeconds 8 -InstallerTimeoutSeconds 300`，退出码为 `0`。
- 隔离验收结果为 `NSIS 发布验收全部通过`：初装、首次启动并退出、隔离 SQLite/Vault 建立、覆盖安装、升级前后数据快照一致、升级后两次独立冷启动、LICENSE/NOTICE Manifest 大小与 SHA-256、静默卸载、测试注册表清理和卸载后 SQLite/Vault 数据保留均通过。
- 本次证据目录为 `.runtime/nsis-acceptance/20260805-local-real/`，摘要文件为 `.runtime/nsis-acceptance/20260805-local-real/acceptance-summary.json`；测试安装目录、Profile、注册表备份和数据回滚备份均限制在 `.runtime` 下，验收结束后安装目录和测试注册表项已清理。
- 该轮是同版本覆盖安装/重装，不是不同版本 Windows 升级；它不能替代第二台不同 `MachineGuid/SID` 机器的跨版本升级、回滚和连续 20 次冷启动门禁。
- 正式 1.0 门禁仍关闭：当前包 `Authenticode=NotSigned`，Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、第二台 Windows、至少五个真实商务案例及同一干净 Git SHA 双平台最终产物仍需外部输入；本轮不提交、不推送、不创建 GitHub Release、不上传正式 R2、不更新 `current` manifest`。

## 49. 2026-08-05 同产品故障注入回滚与旧品牌升级基线拒绝

- 使用同一产品的当前包执行 `-InjectFailureAfterUpgrade` 故障注入：`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/invoke-nsis-release-acceptance.ps1 -InstallerPath <installer> -PreviousInstallerPath <installer> -Version 1.3.4 -RunId 20260805-injected-rollback-same-product -InjectFailureAfterUpgrade -ColdStartCount 2 -StartupObservationSeconds 8 -InstallerTimeoutSeconds 300`。预期注入点触发，脚本退出码为 `1`，不是产品缺陷。
- 注入回滚摘要 `.runtime/nsis-acceptance/20260805-injected-rollback-same-product/acceptance-summary.json` 显示：`status=failed`（故障注入）、`rollbackAttempted=true`、`rollbackCompleted=true`、`rollbackError=null`、`uninstallCompleted=true`、`registryRestored=true`；升级前后的隔离 SQLite/Vault 快照未被改写，受限清理完成。
- 另用仓库现存 `半山商务工作台_1.3.3_x64-setup.exe` 作为候选旧包时，验收脚本在安装后明确拒绝“前一版本安装包不是同一产品”，并完成受限卸载和注册表清理；该包品牌元数据与当前“华邦互娱商务系统”不一致，不能冒充正式跨版本升级基线。摘要见 `.runtime/nsis-acceptance/20260805-injected-rollback/acceptance-summary.json`。
- 因此本机已验证同产品覆盖安装的数据回滚路径，但不同品牌历史包不能证明正式跨版本升级；真正跨版本门禁仍需提供同一产品名的旧版安装包或在第二台干净 Windows 上生成并验收同一产品基线。
## 50. 2026-08-05 发布专项总检回归

- 在当前工作树、`HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb` 和 `CARGO_BUILD_JOBS=1` 下重新执行 `pnpm release:verify`，命令退出码为 `0`。
- 本轮通过项包括：业务技能与协议生成、前端 `328` 个 Rust 协议测试、前端 TypeScript/Vite 构建、Rust fmt/check/clippy/test、Codex sidecar 校验、macOS 发布证据规则、正式发布证据规则、发布工作流策略、R2 发布事务、source snapshot、Windows secondary-machine acceptance、data migration rollback acceptance，以及 `git diff --check`。
- 发布专项没有因第 48/49 节新增的本机验收证据产生回归；工作树仍保留用户既有约 `226` 项修改/删除/未跟踪文件，不执行 reset、clean、提交、推送或大范围覆盖。
- 正式 1.0 外部门禁状态不变：本机受限测试与同产品故障注入回滚已通过，但 Windows Authenticode、第二台不同 `MachineGuid/SID` 的真实 Windows、macOS ARM64/Developer ID/公证/Gatekeeper、五个真实商务案例和同一干净 Git SHA 双平台最终产物仍未具备；不发布、不更新 GitHub Release、R2 正式对象或 `current` manifest`。
## 51. 2026-08-05 macOS App Bundle 资源路径修复与发布总检复跑

- 修复 `src-tauri/src/codex_host.rs` 的非 Windows Codex runtime 发现逻辑：标准 macOS App Bundle 由 `Contents/MacOS/<app>` 启动时，新增检查 `Contents/Resources/codex-runtime/codex`，与 `.github/workflows/build-macos.yml` 的实际资源落位一致；同时加入路径回归测试 `macos_bundle_runtime_uses_contents_resources_directory`，并限制辅助函数的编译条件，避免 Windows 非测试构建产生 dead-code warning。
- 本机验证：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` 通过；`CARGO_BUILD_JOBS=1 cargo check --manifest-path src-tauri/Cargo.toml` 通过；目标 Rust 回归测试通过（1 passed）；随后 `CARGO_BUILD_JOBS=1 pnpm release:verify` 退出码 `0`。
- 本机发布输入探测：Windows SDK `signtool.exe` 存在，但当前用户和本机证书存储均无可用私钥签名证书；Apple `sw_vers`、`xcodebuild`、`codesign`、`xcrun`、`spctl`、`security`、`hdiutil`、`stapler`、`notarytool` 和 GitHub CLI 均不可用。因此本轮不能生成或宣称 Windows Authenticode、macOS ARM64/Developer ID/公证/Gatekeeper、第二台 Windows、真实商务案例及同一干净 SHA 的正式跨平台发布证据。
- 本轮仍不提交、不推送、不创建 GitHub Release、不上传正式 R2、不更新 `current` manifest`；以上外部门禁继续关闭，等待真实签名材料、Apple runner/设备和第二台 Windows 等外部输入。

## 52. 2026-08-05 macOS readiness 文档校正与发布链专项复核

- 将 `docs/MACOS_RELEASE_READINESS_20260729.md` 中已过时的“Codex 资源发现路径未修复”改为准确状态：代码路径修复和 Windows 侧回归已完成，仍需真实 macOS ARM64 runner 完成 Codex/Brain、签名、公证、Stapler、Gatekeeper 和冷启动验证；macOS ARM64 FFmpeg/FFprobe runtime 仍是媒体能力阻塞。
- Windows workflow 静态复核确认正式链包含 PFX 导入、证书 SHA-1 钉扎、`signtool` SHA-256 签名、时间戳、签名验证、PowerShell Authenticode 状态复核和证据封装；本机没有签名私钥，未执行正式签名。
- `pnpm test:macos-release-evidence`：18/18 通过；`pnpm test:release-workflow-policy`：13/13 通过；`git diff --check` 通过。未触发任何发布、推送、提交、GitHub Release、R2 正式对象或 `current` manifest 更新。


## 53. 2026-08-05 macOS 构建日志闭环与完整发布门禁复跑

- macOS workflow 现把 `pnpm tauri build` 的完整输出保存为 `macos-build.log`，并把 App/DMG 的 `codesign`、`spctl` 与 `stapler` 直接校验输出保存为 `macos-gate-checks.log`；两个日志均纳入 `macos-release-manifest.json`、精确文件集合、`SHA256SUMS-macos.txt` 和 GitHub Artifact，避免只记录结论而没有原始构建与门禁输出。
- `scripts/verify-macos-release-evidence.mjs` 已同步强制验证两个日志为发布证据目录内的普通非空文件，并逐项核对名称、大小和 SHA-256；测试 fixture 的证据闭包同步扩展为六个文件，新增代码通过 `node --check`。
- 专项验证结果：`pnpm test:macos-release-evidence` 为 `18/18` 通过，`pnpm test:release-workflow-policy` 为 `13/13` 通过，`git diff --check` 退出码为 `0`；新增的 `spctl --assess --type open` 和日志证据未破坏 workflow 发布策略。
- 在 `HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb`、约 `226` 项既有工作树变化和 `CARGO_BUILD_JOBS=1` 下再次执行 `pnpm release:verify`，完整命令退出码为 `0`。业务技能、协议生成、前端测试/类型检查/构建、Rust fmt/check/clippy/test、Codex sidecar、macOS/正式发布证据、workflow 策略、R2 事务、源码快照、第二台 Windows 验收脚本、数据迁移回滚验收及差异检查全部通过。
- 正式 1.0 状态仍为外部门禁未关闭：缺少 Windows Authenticode 私钥、第二台不同 `MachineGuid/SID` 的真实 Windows、macOS ARM64 runner/设备、Developer ID 与 Apple 公证输入、真实 Gatekeeper/启动/Codex Brain 烟测、至少五个真实商务案例，以及同一干净 Git SHA 生成的最终双平台产物。本轮不提交、不推送、不创建 Release、不上传正式 R2 immutable 资产，也不更新 `version.json`、`version-mac.json` 或 `current` manifest。


## 54. 2026-08-05 Release cleanup ownership race fix and full gate rerun

- Audit found a real partial-success risk in the formal release workflow: GitHub API may create a Tag/Release or partial assets while gh release create still returns failure. If cleanup ownership is armed only after the API call, ERR/INT/TERM rollback cannot remove the residue. RELEASE_OR_TAG_CLEANUP_REQUIRED=1 is now armed before gh release create "$TAG"; the newly added inline comment was removed.
- Updated scripts/verify-release-workflow-policy.mjs and scripts/verify-r2-release-transaction.mjs to validate the effective shell assignment sequence. Comment lines and if false; then dead branches are ignored, and the last effective ownership assignment before Release creation must remain 1. This closes dead-branch spoofing, pre-create reset, and comment-marker false negatives.
- Regression coverage:
  - scripts/verify-release-workflow-policy.test.mjs: 16/16 passed, including post-create ownership, disabled-branch spoofing, pre-create reset, and rollback order.
  - scripts/verify-r2-release-transaction.test.mjs: 8/8 passed, covering the same ownership and R2 transaction constraints.
- Syntax and whitespace checks passed: all four modified scripts/tests passed node --check; git diff --check passed. No commit, reset, clean, push, or overwrite of other user changes was performed.
- With HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb and CARGO_BUILD_JOBS=1, pnpm release:verify completed with process exit code 0. Skills, protocol generation, frontend 328 Rust tests, Vitest/TypeScript/Vite, Rust fmt/check/clippy/test, Codex sidecar, macOS evidence, formal-release, release workflow policy, R2 transaction, source snapshot, secondary Windows acceptance, data migration rollback, and git diff --check all passed. Raw output is saved at .runtime/release-verify-20260805-cleanup-ownership-final.log.
- Formal 1.0 external gates remain closed: Windows Authenticode private key, a second Windows machine with different MachineGuid/SID, macOS ARM64 runner/device, Developer ID and Apple notarization inputs, real Gatekeeper/cold-start/Codex Brain evidence, at least five real business cases, and final dual-platform artifacts from one clean Git SHA are still unavailable. Therefore this round does not publish, create a GitHub Release, upload formal R2 immutable objects, or update version.json, version-mac.json, or the current manifest.

## 55. 2026-08-05 正式 macOS 证据 schema 对齐与 Artifact 闭包修复

- 对照总计划第 18–20 节继续审计统一正式 1.0 证据链，发现 `scripts/verify-formal-release-evidence.mjs` 使用了测试夹具自造的旧 macOS schema（`platform=macos-arm64`、`nativeGates`），而真实 `.github/workflows/build-macos.yml` 生成的是已经由 `scripts/verify-macos-release-evidence.mjs` 验证的 workflow schema（`platform=macos`、`gates`、`evidence`、`files`、`SHA256SUMS-macos.txt`）。旧实现会导致真实 macOS runner 产物即使签名、公证和 Gatekeeper 全部通过，也无法进入最终 formal-release 汇总。
- `scripts/verify-formal-release-evidence.mjs` 现直接复用 `verifyMacosReleaseEvidence`，以真实工作流 schema 校验版本、完整 Git SHA、ARM64 target、Developer ID/codesign、公证、Gatekeeper、App/DMG Stapler、Codex sidecar、启动烟测、构建日志、门禁日志、精确文件集合和 SHA-256 闭包，再绑定最终 DMG；formal verifier 不再维护第二套漂移的 macOS 证据协议。
- 审计同时发现 macOS manifest 和 `SHA256SUMS-macos.txt` 已要求 `macos-build.log`、`macos-gate-checks.log`，但 GitHub Artifact 上传列表漏掉这两个文件。`.github/workflows/build-macos.yml` 已补齐上传，避免 runner 本地验证通过、下载后的最终证据目录却不完整。
- `scripts/verify-formal-release-evidence.test.mjs` 已改用真实 macOS workflow fixture；`scripts/verify-macos-release-evidence.test.mjs` 新增静态回归，强制 Artifact 上传步骤包含 smoke、build、gate-checks、manifest 和 checksum 文件。专项结果：formal-release evidence `8/8` 通过，macOS evidence `19/19` 通过，release workflow policy `16/16` 通过，R2 transaction `8/8` 通过，相关 `node --check` 和 `git diff --check` 均通过。
- 在 `HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb`、约 `226` 项既有工作树变化和 `CARGO_BUILD_JOBS=1` 下执行 `pnpm release:verify`，完整命令退出码为 `0`。原始日志保存于 `.runtime/release-verify-20260805-formal-evidence-schema.log`；业务技能、协议生成、前端测试/类型检查/构建、Rust fmt/check/clippy/test、Codex sidecar、macOS/正式发布证据、workflow 策略、R2 事务、源码快照、第二台 Windows 验收和数据迁移回滚门禁全部通过。
- 正式 1.0 外部门禁仍保持关闭：本机没有 Windows Authenticode 私钥、第二台不同 `MachineGuid/SID` 的 Windows、macOS ARM64 runner/设备、Developer ID 与 Apple 公证输入、真实 Gatekeeper/启动/Codex Brain 证据、至少五个真实商务案例，以及同一干净 Git SHA 的最终双平台产物。本轮不提交、不推送、不创建 GitHub Release、不上传正式 R2 immutable 对象，也不更新 `version.json`、`version-mac.json` 或 `current` manifest。

## 56. 2026-08-06 macOS 原生证据直通、启动日志与晋级策略闭环

- 继续对照总计划第 19.1 节审计唯一正式发布工作流，确认 `.github/workflows/promote-business-workbench-1.0.yml` 仍把真实 macOS evidence 复制到 `MACOS_COMPAT_DIR`，再把 `platform=macos`、`gates` 改写成旧 `platform=macos-arm64`、`nativeGates` 兼容 schema。该步骤既与第 55 节已统一的 formal verifier 冲突，也会在改写 `macos-release-manifest.json` 后使原有 `SHA256SUMS-macos.txt` 失效，真实 runner 产物无法晋级。
- promotion workflow 已删除 `MACOS_COMPAT_DIR`、复制和 schema 改写逻辑，`verifyFormalReleaseEvidence` 现在直接接收 `process.env.MACOS_EVIDENCE_DIR`，原生 manifest 与 checksum 全程只读直通。`scripts/verify-release-workflow-policy.mjs` 新增强制约束：禁止兼容目录、`nativePlatform`/`nativeGates` 和 `native.platform='macos-arm64'` 重写；`scripts/verify-release-workflow-policy.test.mjs` 新增回归，重新插入旧转换时必须失败。
- `.github/workflows/build-macos.yml` 的 12 秒启动存活检查现会在 `kill -0 $PID` 成功后向 `macos-smoke.log` 写入明确非空成功标记，避免 GUI 应用没有 stdout/stderr 时生成零字节 smoke evidence。`scripts/verify-macos-release-evidence.test.mjs` 已增加 workflow 静态断言，确保成功标记位于存活验证之后。
- 专项验证全部通过：macOS evidence `20/20`、formal-release evidence `8/8`、release workflow policy `17/17`、R2 transaction `8/8`；三个相关测试/验证脚本通过 `node --check`，`git diff --check` 退出码为 `0`。
- 在 `HEAD=206bc8360616a9b87b91dac654d5ada4c361ecfb`、约 `226` 项既有工作树变化和 `CARGO_BUILD_JOBS=1` 下执行 `pnpm release:verify`，完整命令退出码为 `0`。原始日志保存于 `.runtime/release-verify-20260806-native-macos-promotion.log`；业务技能、协议生成、前端测试/类型检查/构建、Rust fmt/check/clippy/test、Codex sidecar、macOS/正式发布证据、workflow 策略、R2 事务、源码快照、第二台 Windows 验收脚本和数据迁移回滚门禁全部通过。
- 正式 1.0 仍未达到可发布状态：Windows Authenticode 私钥、第二台不同 `MachineGuid/SID` 的真实 Windows、macOS ARM64 runner/设备、Developer ID 与 Apple 公证、真实 Gatekeeper/启动/Codex Brain/基础任务证据、至少五个真实商务案例、以及同一干净 Git SHA 生成的最终双平台产物仍缺失。本轮继续不提交、不推送、不创建 GitHub Release、不上传正式 R2 immutable 对象，也不更新 `version.json`、`version-mac.json` 或 `current` manifest。

## 57. 2026-08-06 同事试用包、独立 KEY 注入与 Windows 隔离烟测

- 按“安装包脱敏、KEY 独立交付”的原则重新执行 `scripts/build-internal-preview.ps1`。构建前门禁分别拦截了历史 `.runtime/internal-preview-build.json` 中的 API KEY 和本地 `src-tauri/resources/r2.config.json` 中的 R2 凭据；随后使用独立空配置，并在构建期间临时写入 public-only R2 配置，最终通过 `finally` 和 SHA-256 对照恢复原文件，任何 KEY 均未进入安装包、构建日志或 Git 跟踪文件。
- `CARGO_BUILD_JOBS=1` 下完整 `pnpm release:verify` 与 `pnpm tauri build --bundles nsis` 均通过。新 Windows 安装包为 `华邦互娱商务系统_1.3.4_x64-setup.exe`，大小 `132183606` bytes，SHA-256 为 `6b8bbd97b76387d75c978c859425883a4a63bcc9f362d6733b97184ed0c757d7`，Authenticode 状态为 `NotSigned`；build manifest 明确记录 `embeddedInternalApiKey=false`、`bundledR2Credentials=false`。
- 使用 `scripts/invoke-nsis-release-acceptance.ps1` 在完全隔离的安装目录和 Profile 中完成首次静默安装、首次启动、同版本覆盖安装、2 次独立进程冷启动、SQLite Ledger/Local Vault 建立与哨兵保留、Codex LICENSE/NOTICE 校验、静默卸载和测试注册表恢复。`runId=colleague-smoke-20260806-a`，summary 状态为 `passed`；该结果是同事试用包冒烟证据，不替代第二台真实 Windows 或真实跨版本升级证据。
- 新增 `scripts/new-ai-key-injector-bundle.ps1`。该生成器只从被 `.gitignore` 排除的 `.runtime` 配置读取 KEY，仓库源码不包含实际秘密；生成的 `双击注入KEY.cmd` 和 `install-key.ps1` 只存放在隔离交付目录。注入器复用现有 `credentials/provider-key.dpapi` schema v2，以 Windows 当前用户 DPAPI 加密写入、保留旧加密文件备份并在落盘后立即解密自检。
- KEY 注入器已在临时 DataRoot 完成真实自测：schemaVersion=2、defaultProvider=bsaigc、KEY/Base URL/model 均写入成功，磁盘密文不包含明文 KEY；PowerShell 脚本和说明文件统一使用 UTF-8 BOM，双击入口先切换 UTF-8 code page，避免中文乱码。
- 本轮同意将代码提交并推送 GitHub，以同一提交触发既有 macOS ARM64 签名/公证工作流；独立 KEY 注入包继续严格排除在 Git、GitHub Actions Artifact、GitHub Release 和 R2 之外。
