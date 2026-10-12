# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 全部停止并汇总清理错误：待誊写

- 阶段：[roadmap.md](roadmap.md) M3；需求：[requirements.md](requirements.md) P07、P04、P02；设计：[DESIGN.md](../DESIGN.md) §3、§4。
- 目标：集中停止 Manager 当前登记的全部任务，先通知全部再等待；任一运行或清理错误不阻断其他任务的等待，错误带服务 ID 汇总返回。
- 改动：manager.rs 增加 stop_all，持有 ID 快照，复用 request_stop 和 wait；main.rs 的两个前台信号分支改为调用 stop_all 后返回。当前 CLI 仍仅运行一项，本步建立多项收尾能力，不新增 --all 命令。
- 验收：空登记及重复停止成功；未开始执行的任务也收到停止；多个启动错误都报告且其他任务完成清理；慢任务不推迟其他任务收到停止；取消批量等待不丢失未验收的任务，正常任务可继续等待并在完成后复用 ID。
- 教学代码验证：隔离副本 `/tmp/devctl-stop-all-step.uoikife3` 中 `CARGO_TARGET_DIR=/tmp/devctl-retry-step.2rsnb361/target cargo test --locked --offline` 66 项通过（63 项基线 + 3 项全部停止测试）；同环境 `cargo clippy --locked --offline --all-targets -- -D warnings` 和 `cargo fmt --check` 通过。当前源码未应用此步，等待用户誊写；新增测试保留待正式复用。
- 故障注入：`/tmp/devctl-stop-all-fault.pwvxk4ih` 同环境运行 `cargo test --locked --offline stop_all_keeps_uncertain_ids_and_joins_other_tasks` 通过；模拟清理确认失败，失败项保留 ID，其他项完成等待并可复用。
- 边界：独占借用 Manager 期间不接受新 start；每项停止任务并发执行，结果按 ID 顺序等待。取消批量等待后已验收的成功项可能已释放；剩余登记保留，已消费的错误结果尚不缓存。清理未知项不能通过再次 stop_all 重新清理；自动完成事件和后台停止状态后续实现。
- 下一步：验收全部停止后推进多项完成通知和管理循环，再接入多项 CLI、运行锁与后台；后台循环不能直接长时间等待此收尾方法。

## 上一步验收：区分可安全重试与清理未确认的错误

- 基线：`157d187`；用户已完成 process.rs、service_task.rs 与 manager.rs 的功能代码誊写。
- 审查：spawn 前失败可安全重试；spawn 后无法确认资源状态及 JoinError 保留登记；观察错误仅在清理成功后标记可重试；Manager 保留类型至释放判断完成。
- 修复：stop 的 kill_and_reap 结果漏写 `?`，可能忽略清理失败并错误报告停止成功，已补上；登记器仍保留旧错误占用 ID 断言，已迁移至失败后安全重试的行为。
- 测试：正式复用上一轮隔离副本 3 项测试，覆盖无命令、无解析目录、无效目录、非零退出后复用 ID 和任务 panic；保留原有失败隔离、取消等待和进程组测试。
- 验证：宿主 `cargo test --locked --offline` 63 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与本轮文档本地链接检查通过。
- 故障注入：`/tmp/devctl-cleanup-fault.0k8wtidh` 中，使用上述 CARGO_TARGET_DIR 运行 `cargo test --locked --offline cleanup_failure_keeps_the_id_reserved`，修复版本通过；对照移除 `?` 后测试失败并出现 unused Result 警告。注入在实际子进程已清理后返回模拟确认错误，避免测试残留进程。
- 限制：未模拟内核实际清理超时、/proc 权限错误或清理成功后的观察错误；清理未知项仍保留登记，尚无再次清理接口。真实 npm/pnpm 流程待试用。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
