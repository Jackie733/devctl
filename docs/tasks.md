# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：WSL 试用反馈，默认配置发现

- 用户报告 autostart=true 时进入 TUI 未启动，并要求省略 --config，支持 devctl validate / devctl tui。已解释并保留现有约定：autostart 是默认启动组合，进入/重连 TUI 不隐式启动；按 a 或 start --all 才启动该组合。
- 本轮对应 [requirements.md](requirements.md) W02、W03、U01、R02。配置查找顺序和错误规则只在 [interaction.md](interaction.md) §1 主要维护；保留显式路径和当前目录优先，增加两个 HOME 配置位置，不合并配置、不修改用户配置。
- 实现：CLI config 改为可选；main 在命令入口统一选择路径，之后沿用既有加载/规范化、锁和 IPC。后台启动器继续显式传入规范化路径，默认与显式路径可连接同一实例。无参数帮助、--help/--version 不要求配置；发现失败直接报错，不进入 TUI 或创建后台。
- 已更新 README、WSL 安装指南、需求与设计；WSL 获取本次提交并重新 cargo install 后才能使用新查找行为。功能提交见 Git，不自动推送。

## 本轮验证

- `cargo test --locked --offline --test config_discovery_cli --test cli`：新增 8 项查找测试与原有 7 项 CLI 测试通过，覆盖两个 HOME 位置、优先级、显式错误不回退、目录/损坏符号链接、无配置反馈、HOME 缺失和帮助命令。
- 宿主 `cargo test --locked --offline`：**159 项通过**。新增真实 PTY 场景覆盖从无关目录执行短命令 tui、两个 HOME 默认位置、与显式客户端连接同一实例、进入不自动启动。原有后台/日志/进程与 TUI 测试均通过。
- 本轮 Cargo 命令使用 CARGO_HOME=/tmp/devctl-tui-step1-ta02nj4m/cargo-home；Unix socket、信号与 PTY 在允许这些操作的 Linux 宿主执行。`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`cargo build --release --locked --offline`、`git diff --check` 均通过；6 份文档的 32 个本地链接与 WSL 指南 sh 命令块语法检查通过。

## 必要交接与下一步

- M1–M5 的首版功能及受控验证已完成，TUI 提交 feaa688，WSL 指南初版 d215d67。此前已通过 150 项完整回归；本轮是在真实试用反馈上补充配置发现。
- 用户目标为 Windows WSL2；三个目录是 ~/code/deepmdt-frontend-pc、~/code/deepmdt-miniprogram-pro、~/code/rtc-middleware/api-proxy，配置示例见 [wsl.md](wsl.md)。目标 WSL 的发行版、架构、脚本/依赖及 stdin 交互要求仍待核实；本宿主没有这三个目录。
- 继续 M6 的真实 npm/pnpm 流程验收，不能用本机测试通过代替用户体验确认。保留已有 Linux/后台保证与限制：有界近期日志，清理未知不虚报退出，不自动恢复 SIGKILL/断电或 WSL 关闭后的进程，不提前实现其他扩展。
