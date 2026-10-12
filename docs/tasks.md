# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 等待任意任务完成并交付结果：待誊写

- 阶段：[roadmap.md](roadmap.md) M3；需求：[requirements.md](requirements.md) P01、P02、P04；设计：[DESIGN.md](../DESIGN.md) §3、§4。
- 目标：Manager 无需指定服务 ID，即可取得任一任务的完成结果；较早 ID 仍运行不阻塞其他项反馈；沿用安全释放规则，每个结果只消费一次。
- 改动：ServiceTask 增加 is_joined、poll_wait，用 poll_fn 复用等待和 JoinError 分类；Manager 增加 wait_next，用同一 Context 检查未 join 的句柄，抽取 finish_wait 统一释放判断；main.rs 改为接收返回的 ID 和结果。
- 返回约定：Some((id, result)) 表示一次已消费结果；尚有未 join 的任务但均未完成时挂起；None 表示没有未 join 的任务，不表示所有进程组已确认清理或全部 ID 可复用。清理未知项保留登记，后续 wait_next 跳过其已消费句柄。
- 验收：空集合立即返回 None；较后 ID 的完成不等待前项退出；启动失败、退出码、信号和停止结果保留；取消等待后可继续等待或发送停止；按 ID 等待与任意等待不重复消费结果；ID 安全复用后不收到旧结果。
- 教学代码验证：隔离副本 `/tmp/devctl-next-completion-step.i8japint` 中 `CARGO_TARGET_DIR=/tmp/devctl-retry-step.2rsnb361/target cargo test --locked --offline` 70 项通过（66 项基线 + 4 项完成等待测试）；同环境 `cargo clippy --locked --offline --all-targets -- -D warnings` 和 `cargo fmt --check` 通过。当前源码未应用此步，等待用户誊写；新增测试保留待正式复用。
- 故障注入：`/tmp/devctl-next-completion-fault.l1rerq2r` 同环境运行 `cargo test --locked --offline wait_next_reports_uncertain_results_once_and_keeps_their_ids` 通过；模拟清理确认失败及 task panic，结果各报告一次，ID 保留，后续等待不重复 join，并仍能接收其他项和新任务的结果。
- 机制与边界：JoinHandle::poll 注册等待者的 waker，完成后唤醒，不使用周期扫描或增加结果队列；Pending 时保留句柄，Ready 后才清空。检查按 ID 顺序进行，不保证多个已完成结果按退出时间排序。结果消费至登记更新之间没有 await；取消等待仍不缓存已消费的错误原因，清理未知项暂不能再次清理。客户端界面状态及实例事件标识尚未实现。
- 下一步：验收任意完成等待后接入多项前台管理循环和启动选择，再推进运行锁、后台与 TUI；后台控制循环的全部停止需通过状态和完成结果推进。

## 上一步验收：全部停止并汇总清理错误

- 基线：`855f362`；用户已完成 manager.rs 与 main.rs 誊写。
- 审查：快照建立及全部通知间无 await，独占借用保证登记不变；通知全部后逐项等待，各清理任务并发执行；错误汇总返回，不提前跳过其他项；前台信号分支等待收尾后返回。未发现功能性誊写问题，无功能修复。
- 测试：正式复用上轮隔离副本 3 项测试，覆盖空登记、重复停止、尚未执行的任务、多错误汇总、慢任务不推迟其他停止请求、取消后恢复等待和 ID 复用；实际确认快任务与强杀后的慢任务直接子进程均已回收。
- 验证：宿主 `cargo test --locked --offline` 66 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与本轮文档本地链接检查通过。
- 限制：当前 CLI 仍运行单项；批量等待被取消时，已消费的错误原因尚不缓存，清理未知项不能通过再次停止重新清理。内核实际清理超时、/proc 权限错误与真实 npm/pnpm 流程仍待验证。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
