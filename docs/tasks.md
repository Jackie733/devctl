# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 后台内部入口与空闲生命周期：待誊写

- 阶段：[roadmap.md](roadmap.md) M4；需求：[requirements.md](requirements.md) P05、R02、P04；设计：[DESIGN.md](../DESIGN.md) §6。
- 上一步代码已提交 `f17120c`，本轮检查没有新的源码改动或待修复内容。任务 3 恢复教学誊写流程，本轮只更新任务记录。
- 本步目标：增加隐藏内部命令 supervise 和 supervisor::run。规范化配置路径、加载配置、注册 SIGINT/SIGTERM、取得 RunLock，再创建空闲 Manager；不调用 start_all，不因没有运行服务而退出。收到信号后完成收尾并释放锁。
- 边界：本步内部入口仍以前台方式调试；尚未脱离控制终端、重定向标准流、建立启动握手或绑定控制端点，不宣称后台运行已完成。supervise 不作为新的面向用户的操作入口；后续由客户端启动该内部进程。
- 教学代码验证：隔离副本 `/tmp/devctl-supervisor-step.xqp3ax44` 使用其独立 target。宿主 `cargo test --manifest-path /tmp/devctl-supervisor-step.xqp3ax44/Cargo.toml --target-dir /tmp/devctl-supervisor-step.xqp3ax44/target --locked --offline` 91 项通过（89 项基线 + 2 项内部入口测试）。副本的 `cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。
- 新测试验证：包含 autostart 项时仍不执行命令；空闲持锁；拒绝重复管理者与前台 run；validate/plan 可用；SIGINT/SIGTERM 均成功退出并允许重新取得锁；无效配置在取锁前失败；内部命令不出现在顶层帮助。测试保留在副本 tests/supervisor_cli.rs，下一轮审查誊写时复用，不让用户誊写测试。
- 环境差异：本轮沙箱内信号退出测试超时，宿主完整测试通过；沙箱结果未计为通过，根因未定位。
- 下一步：本步誊写验收后，继续进程脱离终端与启动握手；控制端点就绪前不能对客户端报告可用。再接控制通道、实例发现与控制 CLI。
- 后台复用 RunLock 与 Manager：创建不自动启动项目；客户端离开不停止服务。当前内部入口只等待退出信号，stop_all 仅用于最终收尾；接入控制请求后用 request_stop_all + wait_next 推进停止，避免阻塞控制循环。

## 上一步验收：运行锁与多项目控制核心

- 关联：M3；R02、P01、P07、P02、P04。基线：`8f63797`；源码与测试由助手直接编写，非待誊写代码。
- 任务 1：RunLock 使用标准库 File::try_lock，按规范化配置路径追加 .devctl.lock；先完成校验和信号注册，再持锁启动。锁文件不截断、不自动删除，拒绝符号链接与非普通文件；validate/plan 不争锁。
- 任务 2：Manager 保留服务定义和 Stopped/Starting/Running/Stopping/Failed 状态。整批预检；重复启动幂等；单项停止不影响其他项；默认组合启动、全部停止、停止后显式重启；启动成功有独立报告，失败原因保留到新一轮启动。全部停止请求不 await，完成结果推进状态，整批完成前禁止新启动。
- 安全边界：清理未知保留登记及原始错误、禁止重复启动；后续停止/等待不虚报成功，也不覆盖原始原因。stop_all 汇总错误并继续收尾其他项。未实现对未知残留再次清理或自动恢复，不根据旧 PID 发信号。
- 可见入口：新增 `run --all`，选择 autostart 默认组合，与显式 ID 互斥；空组合成功退出。运行中的单项控制/状态查询已由 Manager 测试验证，尚无独立 start/stop CLI；这些入口随任务 3 的后台通信接入，不能据此宣称完整产品流程已完成。
- 测试：复用 75 项基线，新增 4 项锁组件测试、3 项跨进程锁测试、6 项 Manager 控制测试、1 项 --all CLI 测试，共 89 项。覆盖别名、原子替换、并发竞争、不同配置、只读命令、停止期间持锁、句柄释放，以及单项/整批控制、预检、状态、失败保留、取消等待和清理未知。
- 验证：宿主 `cargo test --locked --offline` 89 项通过；`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。`cargo fmt --check`、`git diff --check` 和 5 份改动文档的本地链接目标检查通过。
- 环境差异：沙箱完整并行测试两次在 multi_run_cli 的信号停止用例超时，单独运行通过；宿主完整测试通过。未将沙箱超时结果记为通过，具体环境原因尚未定位。
- 限制：锁要求 Rust 1.89+，本机为 1.98.1；仅验证当前 Linux 本机文件系统，网络文件系统与硬链接配置别名互斥不在已验证范围。后台、日志缓存、TUI 尚未实现；真实 npm/pnpm 流程及内核实际清理超时、/proc 权限错误仍待验证（清理未知的管理边界使用注入错误测试）。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
