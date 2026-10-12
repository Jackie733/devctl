# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 将每项运行封装为 ServiceTask：待誊写

- 阶段：[roadmap.md](roadmap.md) M3 的准备；需求：[requirements.md](requirements.md) P01、P07、P02、P04；设计：[DESIGN.md](../DESIGN.md) §1、§3、§4。
- 目标：每项运行用一个 ServiceTask 保存服务 ID、停止发送端和任务句柄；提供独立的停止请求与结果等待，为多项目控制准备具体句柄。
- 改动：新增 service_task.rs；spawn 接收拥有所有权的 Service 与 ID，request_stop 最多发送一次请求，wait 借用句柄，取得结果后移除句柄并保留错误反馈；run_service 使用该接口。
- 验收：两项长运行任务可以同时启动；停止一项或另一个任务启动失败不影响其余任务；重复停止不重复发送；取消一次等待后可继续等待同一任务；现有单项前台行为保持一致。
- 教学代码验证：`/tmp/devctl-service-task-step.TQQjdU` 隔离副本中，宿主 `cargo test --locked --offline` 57 项通过（55 项基线 + 2 项控制隔离测试）。修复测试代码的 Clippy 格式建议后，`cargo test --locked --offline --bin devctl service_task::tests` 2 项复测通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。当前源码未应用此步，等待用户誊写后正式验收；隔离副本保留待复用的测试。
- 边界：spawn 只代表创建运行任务，不代表开发进程已启动；wait 返回错误不能当作清理成功。尚无多项 CLI、重复启动保护、运行锁或后台；任务 panic 与强制结束运行时的完整清理仍未保证。
- 下一步：验收每项控制句柄，再将多个任务纳入统一管理，增加服务 ID 的重复启动保护和任务完成处理。

## 上一步验收：将单次运行放入独立 Tokio 任务

- 基线：`866b1f1`；用户已完成 main.rs 誊写，内容与教学代码一致。
- 审查：Service 与 ID 由 async move 移入任务；入口保留发送端和 JoinHandle；信号分支通知停止后等待同一任务；JoinError 与运行错误分别传播。正常路径没有丢弃句柄或 abort，未发现需修复的功能问题。
- 测试：新增 1 项，验证在单线程运行时中，任务尚未执行时发送的停止请求仍能被处理，返回 Stopped。复用启动失败、意外退出、信号停止、TERM 宽限期和进程组清理测试。
- 验证：宿主 `cargo test --locked --offline` 55 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与文档本地链接检查通过。
- 限制：Linux /proc 可见性；清理超时、读取权限错误与任务 panic 未做故障注入；真实 npm/pnpm 项目仍待试用。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
