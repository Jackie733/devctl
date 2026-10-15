# devctl

Rust 开发项目管理工具：在一个终端界面中启动 web、小程序和 Node.js 中台，查看各项目日志、单独或全部启停；离开界面后后台继续运行，结束开发时统一停止。

当前已实现配置校验、依赖计划、配置运行锁、前台运行，以及独立后台的单项/全部启停、状态查询和有界项目日志。关闭客户端终端后服务继续运行，后续命令连接原后台。TUI 已支持项目选择、单项/全部启停、近期日志滚动、重连和结束工作区。实际验收进度见[开发与学习任务记录](docs/tasks.md)；下列规划文档描述目标能力，不代表全部已经实现。

## 项目规划

- [协作与 Rust 教学规范](AGENTS.md)：逐步审查、测试、提交与誊写学习流程。
- [开发与学习任务记录](docs/tasks.md)：实际进度、验证证据与下一步。
- [需求与验收标准](docs/requirements.md)：产品边界、优先级和验收标准。
- [交互流程](docs/interaction.md)：首版终端界面、单项与全部启停、离开与结束工作区。
- [技术架构](DESIGN.md)：配置、后台控制、并发进程管理、可靠停止与有界日志。
- [实施路线](docs/roadmap.md)：首版阶段顺序、需求映射与学习重点。

开发主线：跨目录配置 → 可靠单项启停 → 多项目管理 → 后台控制 → TUI 与日志 → 真实项目试用。其他扩展在 MVP 完成后再评估，清单见需求文档。

## 当前运行方式

Windows WSL2 的本地 release 安装、三个真实项目配置和试用步骤见 [WSL2 安装指南](docs/wsl.md)。目标 WSL2 尚待实测，不把已有 Linux 测试当作 WSL2 验收结果。

终端界面（将配置路径替换为自己的配置）：

```sh
cargo run -- --config /path/to/devctl.toml
```

配置放在当前目录的 `devctl.toml`、`~/devctl.toml` 或 `~/.config/devctl/devctl.toml` 时，直接运行 `devctl` 进入 TUI，`devctl tui` 也继续可用；`devctl validate` 等子命令执行各自操作，`--help` / `--version` 输出信息后退出。显式 `--config` 优先，其次按上述顺序选择第一个存在的配置；缺少配置会报错。完整规则见[交互方案](docs/interaction.md)。

首次进入可创建空闲后台，不自动启动项目；`autostart = true` 表示纳入 `a` / `start --all` 的默认组合。`j/k` 在当前面板选择项目或移动日志阅读光标，`h/l` 或 Tab 切换面板，`Enter/Esc` 进入日志或返回项目；`gg/G` 跳转首尾，`Ctrl-u/d` 半屏滚动。`s/x` 启停选中项，`a` 启动默认组合、`X` 停止全部；`PageUp/PageDown` 滚动日志，`End` 跟随，`?` 查看可滚动的帮助与完整诊断。日志面板独立显示当前服务状态，停止后明确标为保留日志；底部固定两行；先用 `j/k` 定位日志，再用 `v` 从该行开始整行选择、`j/k` 扩展、`y` 请求复制、`Esc` 取消。本机 WSL 使用 clip.exe，其他环境及 SSH 使用 OSC 52；限制与失败处理见[交互方案](docs/interaction.md#整行选择与复制)，复制后请实际粘贴核对。窄窗口显示活动面板。`q` 或 Ctrl-C 离开后服务保留；`Q` 经确认后结束工作区。

后台控制（将服务 ID 替换为自己的配置）：

```sh
cargo run -- --config /path/to/devctl.toml start web
cargo run -- --config /path/to/devctl.toml start --all
cargo run -- --config /path/to/devctl.toml status
cargo run -- --config /path/to/devctl.toml logs web --limit 100
cargo run -- --config /path/to/devctl.toml stop web
cargo run -- --config /path/to/devctl.toml stop --all
cargo run -- --config /path/to/devctl.toml shutdown
```

`start` 和首次进入 TUI 可按需创建后台；`stop`、`status`、`logs` 和 `shutdown` 连接已有实例。`start` 返回请求已受理，进程结果通过 `status` 和日志查看。`stop --all` 保留后台，`shutdown` 完成清理后结束后台；清理无法确认时返回错误并保留后台及运行锁供排查。已有后台沿用创建时配置和环境，修改配置后需先结束工作区，再重新创建。

`logs` 返回近期输出快照，区分 stdout/stderr、运行实例和日志序号。可用 `--after <序号>` 分页读取后续记录；游标仅对同一后台实例有效。截断、缓存淘汰、队列丢弃或读取不完整会提示。日志只保存在后台的有界内存中，结束后台后不保留历史。

保留前台运行：

```sh
cargo run -- --config /path/to/devctl.toml run web
cargo run -- --config /path/to/devctl.toml run web weapp middle
cargo run -- --config /path/to/devctl.toml run --all
```

启动的 `--all` 选择 `autostart = true` 的默认组合，与显式 ID 互斥。前台用 Ctrl-C/SIGTERM 清理所选服务；前后台共享同一运行锁，不能同时托管同一配置。操作规则见[交互方案](docs/interaction.md)。

运行锁使用 Rust 1.89+ 的标准库文件锁，目前在 Linux 本机文件系统验证。配置目录需可创建锁文件和私有运行目录；运行期间不要删除 `.devctl.lock` 或 `.devctl/`。锁文件存在不代表被占用。详细机制与限制见[设计](DESIGN.md)。

验证命令：`cargo test --locked --offline`、`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`。进程与 Unix socket 测试需要允许创建本地 socket 和发送信号的 Linux 环境，测试还使用 `sh`、`sleep` 与 Python 3 构造受控场景。真实 npm/pnpm 项目流程仍待用户试用。
