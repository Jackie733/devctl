# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 区分可安全重试与清理未确认的错误：待誊写

- 阶段：[roadmap.md](roadmap.md) M3；需求：[requirements.md](requirements.md) P01、P02、P04；设计：[DESIGN.md](../DESIGN.md) §3、§4。
- 目标：启动未创建进程或已确认清理时允许释放 ID；清理未确认时保留 ID，错误仍显示原有原因，不影响其他已登记服务。
- 改动：process.rs 增加 RunError 的 SafeToRetry、CleanupUnconfirmed 两种结果；run、stop 在资源边界分类；ServiceTask 保留类型并将 JoinError 归为清理未确认；Manager 根据类型决定是否释放，最后转换为 String。
- 验收：命令缺失、工作目录无效及 spawn 失败后可使用同一 ID；非零退出仍保留退出码且清理后可重新启动；取消等待、停止未完成时仍拒绝重复启动；任务 panic 不构成清理完成证据。
- 教学代码验证：隔离副本 `/tmp/devctl-retry-step.2rsnb361` 中 `cargo test --locked --offline` 63 项通过（调整 1 项登记器测试，新增 3 项测试）；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。当前源码未应用此步，等待用户誊写；隔离副本保留待复用测试。
- 测试迁移：service_task.rs 的启动失败断言改为检查 `error.to_string()`；manager.rs 原错误保留 ID 测试改为验证启动失败可重试且不影响其他项。用户誊写后复用隔离副本中的工作目录/命令边界、非零退出后复用 ID、任务 panic 测试。
- 边界：清理超时、读取权限错误和清理成功后的观察错误未做故障注入；清理未知的登记仍不支持主动解除或重新清理。start 成功仅表示任务已登记，不表示子进程已启动；本步不增加自动重试。
- 下一步：验收错误分类后推进多项完成处理与全部停止；多项 CLI、运行锁、后台与 TUI 尚未实现。

## 上一步验收：按服务 ID 登记任务并阻止重复启动

- 基线：`832c5d9`；用户已完成 manager.rs 和 main.rs 誊写。
- 审查：重复 ID 和不支持的依赖在 spawn 前拒绝；按 ID 停止，等待成功后释放登记；停止请求和取消等待保留任务；单项错误不停止其他项。
- 修复：main.rs 两个信号分支补上 `manager.request_stop(id)?`，避免忽略 Result；统一 Manager 导入位置。
- 测试：正式复用上轮隔离副本 3 项登记器测试，覆盖重复启动、重复停止、取消等待、停止及自然退出后复用 ID、未知 ID、依赖拒绝与单项失败隔离。
- 验证：宿主 `cargo test --locked --offline` 60 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与本轮文档本地链接检查通过。
- 限制：当前 String 错误无法判断清理状态，wait 错误一律保留 ID；即使进程未创建也暂时不能复用。CLI 仍只托管一项，真实 npm/pnpm 项目待试用。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
