# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：源码版本升级至 1.1.0，准备 WSL 自行构建与试用

- 本轮起点 bfdd69f，工作区干净。上一轮已实现 L01–L04 收敛后的日志增强；本轮按用户要求将 Cargo.toml 与 Cargo.lock 中 devctl 包版本从 1.0.0 升为 1.1.0，依赖版本不变。CLI 与 TUI 顶栏继续使用包元数据，不另加硬编码版本。
- 已将日志变更归入 CHANGELOG.md 的 1.1.0 源码版本，README 明确源码与已有 1.0.0 附件的区别。未创建远程 Release、Git 标签或推送提交。
- [wsl.md](wsl.md) §1–§2 已补齐新版源码核对、工具准备、自动测试、release 构建与安装；统一使用已有更新器安装到 ~/.local/bin，避免 Cargo 路径优先级导致继续执行旧版。构建/测试先完成，最后再由用户显式结束旧后台并切换程序；不改原配置。
- [wsl.md](wsl.md) §4.1 补齐三项目日志保存、暂停/跟随、搜索/复制、重新进入默认跟随、结束后台后查历史及会话隔离的试用流程。操作与存储机制继续由 interaction.md、DESIGN.md 维护。

## 验证证据

- `cargo check --locked --offline`、`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 全部通过。
- `cargo test --locked --offline`：214 项全部通过（131 单元、83 集成，含 10 项真实 PTY）；本轮以 1.1.0 重新运行，不沿用上一轮结果。
- `python3 tests/support/install_scripts.py`：10 项安装/更新/回退与 PATH 测试通过，使用临时 HOME，不修改用户实际安装或配置。
- `cargo build --release --locked --offline` 成功，实际运行 `./target/release/devctl --version` 输出 `devctl 1.1.0`。本机构建使用 Rust/Cargo 1.98.1，不将此结果冒充目标 WSL 的构建或安装验收。
- Markdown 本地链接/锚点/围栏、`git diff --check` 通过；已审查版本改动与安装指令，未修改功能实现或依赖。

## 下一步与交接

1. 将包含本轮版本提交的源码同步到目标 WSL；本地未推送的提交不会通过另一个克隆的 git pull 自动取得，也不能用 v1.0.0 的源码包构建日志新版。
2. 用户按 wsl.md §2 在目标 WSL 运行自动检查并构建；确认版本为 1.1.0 后，方便停止项目时结束旧后台，再安装并按 §4.1 试用。助手未结束现有后台、替换安装程序或操作三个真实项目；WSL 实际体验仍待用户验收。
3. 必要实现交接：bfdd69f 已完成独立有界归档、轮转/期限、磁盘历史、单项目字面搜索与查询页重定向；不实施跨退出书签、跨项目聚合或复杂筛选。保存格式和故障边界见 [DESIGN.md](../DESIGN.md#53-mvp-后日志增强方案)。
4. MVP 1.0.0 的既有发布见 [Release](https://github.com/Jackie733/devctl/releases/tag/v1.0.0)；AI、X03 公开下载及命令内更新继续保留后续讨论，本轮不启动。
