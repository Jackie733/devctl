# devctl

Rust 开发项目管理工具：在一个终端界面中启动 web、小程序和 Node.js 中台，查看各项目日志、单独或全部启停；离开界面后后台继续运行，结束开发时统一停止。

当前已实现配置校验、依赖计划、配置运行锁，以及显式选择一项/多项或默认组合的前台运行。Manager 已提供运行期间单项/批量控制和状态查询的内部接口；后台控制命令与 TUI 尚未接入。实际验收进度见[开发与学习任务记录](docs/tasks.md)；下列规划文档描述目标能力，不代表全部已经实现。

## 项目规划

- [协作与 Rust 教学规范](AGENTS.md)：逐步审查、测试、提交与誊写学习流程。
- [开发与学习任务记录](docs/tasks.md)：实际进度、验证证据与下一步。
- [需求与验收标准](docs/requirements.md)：产品边界、优先级和验收标准。
- [交互流程](docs/interaction.md)：首版终端界面、单项与全部启停、离开与结束工作区。
- [技术架构](DESIGN.md)：配置、后台控制、并发进程管理、可靠停止与有界日志。
- [实施路线](docs/roadmap.md)：首版阶段顺序、需求映射与学习重点。

开发主线：跨目录配置 → 可靠单项启停 → 多项目管理 → 后台控制 → TUI 与日志 → 真实项目试用。其他扩展在 MVP 完成后再评估，清单见需求文档。

## 当前运行方式

```sh
cargo run -- --config /path/to/devctl.toml run web
cargo run -- --config /path/to/devctl.toml run web weapp middle
cargo run -- --config /path/to/devctl.toml run --all
```

`--all` 选择 `autostart = true` 的服务，与显式 ID 互斥；默认组合为空时正常退出。前台的选择、停止与退出规则见[交互方案](docs/interaction.md)。当前仍用 Ctrl-C/SIGTERM 停止前台管理的整批服务，独立 `start`、`stop`、`shutdown` 命令将在后台阶段接入。

运行锁使用 Rust 1.89+ 的标准库文件锁，目前在 Linux 本机文件系统验证。配置目录需可创建锁文件；`配置文件名.devctl.lock` 会保留，运行期间不要删除它，文件存在不代表锁被占用。详细机制见[设计 §4](DESIGN.md#4-进程停止与运行互斥)。
