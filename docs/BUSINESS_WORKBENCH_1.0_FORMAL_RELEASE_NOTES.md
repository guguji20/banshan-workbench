# 半山商务工作台业务版 1.0 正式发布说明

> 状态：发布前草案；全部门禁关闭前不得改为正式批准状态，不得发布。

- 产品版本：业务版 1.0
- 程序版本：1.3.4
- `releaseStatus: blocked-until-all-gates-pass`

## 正式发布门禁

- Windows 与 macOS 必须来自同一个干净 Git 提交。
- Windows Authenticode、macOS Developer ID 签名与 Apple 公证必须通过。
- 两台 Windows、不同用户、累计至少 20 次冷启动、升级回滚和数据完整性必须通过。
- macOS ARM64 启动与基础任务烟测必须通过。
- 至少 5 个真实商务项目必须完成试用并关闭阻塞问题。
- 源码快照、SHA-256、构建日志、验收证据和回滚说明必须齐全。

全部门禁完成后，将本文件状态改为 `releaseStatus: formal-1.0-approved`，补充最终 Git 提交、SHA-256、已知限制和升级说明，再运行唯一正式发布工作流。
