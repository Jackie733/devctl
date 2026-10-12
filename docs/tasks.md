# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 多项前台运行与失败隔离：待誊写

- 阶段：[roadmap.md](roadmap.md) M3；需求：[requirements.md](requirements.md) P01、P07、P02、P04、W03；设计：[DESIGN.md](../DESIGN.md) §3、§4。
- 目标：`run SERVICE...` 显式选择一项或多项，无需等待任一命令结束或业务就绪；重复 ID 只登记一次；单项启动失败或退出立即显示原因，其他项继续运行；前台停止信号集中清理并保留本轮失败退出码。
- 改动：cli.rs 的 Run 参数改为至少一个 String 的 Vec；main.rs 的 run_services 先去重、校验并取得全部 Service 所有权，再统一登记；循环调用 wait_next，逐项显示错误并记录 failed；信号触发 stop_all。登记过程中若意外失败，先等待已登记项清理再返回错误。
- 验收：未知 ID、未支持的依赖在整批任何 spawn 前拒绝；三个受控服务均启动，不等待第一项退出；重复 ID 无第二进程；启动失败、退出码 0/7 或信号退出立即报告且另一项保留运行；SIGINT/SIGTERM 清理整批进程组并回收直接子进程；之前有失败则最终退出码仍为 1。
- 教学代码验证：隔离副本 `/tmp/devctl-multi-run-step.js_h99tb` 中 `CARGO_TARGET_DIR=/tmp/devctl-retry-step.2rsnb361/target cargo test --locked --offline` 75 项通过（70 项基线 + tests/multi_run_cli.rs 的 5 项 CLI 测试）；同环境 `cargo clippy --locked --offline --all-targets -- -D warnings`、`cargo fmt --check` 通过。当前源码未应用此步，等待用户誊写；新测试文件保留待正式复用。
- 边界：至少指定一个 ID，没有默认自动启动或 --all 选项；沿用依赖拒绝，不做依赖执行。当前是前台模式，Ctrl-C 停止托管服务；后台客户端离开语义见 [interaction.md](interaction.md)。失败原因逐项打印，最后汇总失败状态；跨命令互斥运行锁、后台、TUI 与日志缓冲仍待实现。清理未知项暂不能再次清理。
- 下一步：验收多项前台运行后补齐配置运行锁，再推进后台所有权与控制通道；后台全部停止通过状态和完成结果推进，保持控制请求可响应。

## 上一步验收：等待任意任务完成并交付结果

- 基线：`3a12563`；用户已完成 service_task.rs、manager.rs 与 main.rs 誊写。
- 审查：poll_wait 在 Pending 时保留句柄，Ready 后清空并分类 JoinError；wait_next 跳过已消费句柄，其他 Pending 句柄注册 waker；取得结果至更新登记之间无 await；finish_wait 统一释放规则。未发现功能性誊写问题，无功能修复。
- 测试：正式复用上轮隔离副本 4 项测试，覆盖较后 ID 先完成且前项保持运行、取消后停止并接收结果、启动失败/退出码/信号各消费一次、按 ID 与任意等待交叉使用、ID 复用及换一个 Tokio 任务继续等待；保留全部停止与进程组测试。
- 验证：宿主 `cargo test --locked --offline` 70 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与本轮文档本地链接检查通过。
- 限制：当前 CLI 仍只运行一项；None 不代表所有登记已释放，清理未知仍保留 ID。消费后的错误结果不缓存，清理未知项暂无再次清理接口；真实 npm/pnpm 流程、内核实际清理超时和 /proc 权限错误仍待验证。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
