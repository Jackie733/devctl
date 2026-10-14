# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 本地控制端点绑定与清理：待誊写

- 阶段：[roadmap.md](roadmap.md) M4；需求：[requirements.md](requirements.md) P05、R02、P04；机制与取舍：[DESIGN.md](../DESIGN.md) §6。
- 本步只推进私有 Unix socket 的绑定、持有及清理：新增 control_socket.rs；supervisor 在取得运行锁后绑定端点，再创建 Manager。nix 启用已有包的 user feature；不新增依赖包。生成目录拟加入 .gitignore。
- 端点位于“规范化配置路径 + .devctl/control.sock”。复用目录前检查类型、当前有效用户及 0700 权限；只在持有相同配置锁后移除残留 socket，拒绝覆盖其他文件；端点 guard 借用锁，退出仅尽力移除自己的 socket。
- 教学代码已在 `/tmp/devctl-control-socket-step.lu9b90k_` 的独立 target 验证。宿主 `cargo test --manifest-path /tmp/devctl-control-socket-step.lu9b90k_/Cargo.toml --target-dir /tmp/devctl-control-socket-step.lu9b90k_/target --locked --offline` 98 项通过（91 项基线 + 5 项组件测试 + 2 项集成测试）；副本 `cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。
- 新测试覆盖真实连接、目录权限、残留端点替换、非 socket/符号链接拒绝、替换路径保留、过长路径报错；集成测试验证管理入口绑定、竞争者不改动活跃端点、两种信号清理、空闲管理者 SIGKILL 后下次持锁重建端点。测试位于副本 src/control_socket.rs 和 tests/supervisor_cli.rs，下轮验收誊写时正式复用。
- 本步功能代码未应用到项目，等待用户誊写。当前仍以前台方式调试；尚不 accept 或处理请求，没有协议握手或 detached 启动，socket 连接成功不能作为后台就绪证据。文件系统权限检查不防御同一用户主动替换私有目录。
- 次序调整：启动握手需要在端点能够服务请求后报告可用，因此先完成端点所有权，再继续请求处理、脱离终端和有界启动握手，随后接入实例发现与控制 CLI。管理循环用 request_stop_all + wait_next 推进停止，不能让单次清理阻塞控制请求。

## 上一步验收：后台内部入口与空闲生命周期

- 基线：`b4fa79b`；用户已完成 supervisor.rs、cli.rs、main.rs 誊写，与教学代码一致，审查未发现必要修复。
- 行为：规范化配置路径、加载配置、注册 SIGINT/SIGTERM、取得 RunLock、创建空闲 Manager；不自动启动服务，也不因空闲退出。收到信号后 stop_all 收尾，再释放锁。
- 正式落盘 tests/supervisor_cli.rs 两项测试：包含 autostart 项时不执行命令；空闲持锁；拒绝重复管理者及前台 run；validate/plan 可用；SIGINT/SIGTERM 后释放锁并可重建管理者；无效配置取锁前失败；内部命令不出现在顶层帮助。
- 实际验证：宿主 `cargo test --locked --offline` 91 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。本轮未重复沙箱信号测试，沿用宿主进行进程验收；此前沙箱超时原因仍未定位。`git diff --check` 和两份改动文档的本地链接目标检查通过。
- 限制：内部入口尚未脱离终端；后台、日志缓存、TUI 尚未完成，真实 npm/pnpm 使用流程待用户验收。此时没有管理中的服务，空闲管理者崩溃测试不能证明托管进程崩溃恢复。

## 必要交接

- 运行锁与多项目控制核心已提交 `f17120c`。清理未知保留登记和原始错误，禁止该项再次启动；不根据旧 PID 发信号或自动恢复。已确认清理的失败可显式重试。
- 用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务；结束开发时显式全部停止，另有结束后台工作区操作。三个命令无需等待中台就绪。
- 先完成多项目控制与后台，再接入 TUI。需求 §4 的其他扩展在 MVP 完成并试用后评估。测试通过不代替用户完整流程验收。
