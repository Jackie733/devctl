# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 将单次运行放入独立 Tokio 任务：待誊写

- 阶段：[roadmap.md](roadmap.md) M3 的准备；需求：[requirements.md](requirements.md) P01、P07、P04；设计：[DESIGN.md](../DESIGN.md) §1、§3、§4。
- 目标：运行任务拥有服务配置、服务 ID 和停止接收端；入口拥有停止发送端与 JoinHandle，为多项目并发运行建立所有权边界。
- 改动：run_service 从本地配置取出 Service，用 async move 与 tokio::spawn 启动运行；select! 等待任务或终端信号，发出停止请求后继续等待同一任务；分别处理 JoinError 和运行错误。
- 验收：现有 run 单项前台行为保持一致，停止仍等待完整清理，启动失败和意外退出仍显示服务名；任务启动后没有丢弃句柄或 abort 的正常退出路径。
- 教学代码验证：`/tmp/devctl-task-step.UGp0es` 隔离副本中，宿主 `cargo test --locked --offline` 54 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。当前源码未应用此步，等待用户誊写后正式验收。
- 边界：这一步仍只运行一个服务，未实现多项控制、运行锁或后台；不把任务 panic、运行时被强制结束当成已保证完整清理。
- 下一步：验收独立任务后，引入多项运行的任务跟踪与停止控制。

## 上一步验收：将停止请求与终端信号分开

- 基线：`751e15a`；用户已完成 Cargo.toml、main.rs、process.rs 的停止通道誊写。
- 审查：停止请求与发送端丢失都进入现有清理流程；入口在子进程启动前注册信号，select! 借用固定的运行 future，信号到达后等待同一次运行完成。
- 修复：整理 send(()) 和 Cargo.toml 的格式；无需功能性修复。
- 测试：新增 1 项，分别验证发送请求、丢弃发送端；先确认 leader 与 worker 存活，停止后检查直接 Child 已回收且进程组无存活成员。复用自然退出、TERM 宽限期与后代清理测试。
- 验证：受限环境完整测试中 6 项真实信号场景超时；宿主 `cargo test --locked --offline` 54 项全部通过。`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过；`git diff --check` 通过。
- 限制：Linux /proc 可见性；清理超时、读取权限错误与停止竞态未做故障注入；真实 npm/pnpm 项目仍待试用。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
