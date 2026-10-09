# 实施路线与开源学习

状态：规划，尚未实现。调研日期：2026-10-09。阶段按验收成果推进，不给缺乏团队与投入信息的工期承诺。

## 1. 实施路线

优先验证最难返工的边界：真实进程生命周期、依赖就绪和停止/重试竞态。TUI 在引擎可运行后加入。

| 阶段 | 交付物 | 完成标准 | 主要需求 |
| --- | --- | --- | --- |
| M1 配置和计划 | Spec、TOML 加载、路径解析、图校验；validate/plan CLI | 两个不同目录项目；环和缺失引用报错；无服务被执行 | W01–W03 |
| M2 生命周期 | 前台单服务运行、Runner、Unix 进程组、退出与重启策略、锁 | 派生后代清理；忽略 TERM 强杀；退出码可见；停止取消退避 | P01–P04、R02 |
| M3 编排 | 依赖闭包、并行调度、TCP/exec 就绪、blocked/degraded | DB 延迟就绪、API 崩溃、独立 worker 三类场景通过 | D01–D03 |
| M4 可观察与操作 | 有界聚合日志、过滤搜索、项目/服务树、三级批量操作、Ratatui TUI、no-tui | 高输出仍可操作；独立项目启停与工作区批量控制；窄窗口；终端恢复 | L01–L03、U01–U02、P07 |
| M5 会话恢复与首版 | 状态文件、最近工作区、resume、Linux/WSL2 验收 | 保存/恢复期望服务；损坏状态可解释；完整首版场景通过 | R01、X01 |
| M6 日常体验增强 | 别名/profiles、HTTP 探针、一次性任务、日志轮转 | 迁移成功门槛、历史日志和服务组合有实际使用反馈 | W04、D04、L04 |
| M7 后台与其他平台 | 后台 supervisor、IPC、attach/detach；macOS/Windows | 终端关闭后运行；重连正确；各平台停止和锁测试通过 | P05、X02 |
| M8 经需求验证的扩展 | 持续健康、级联操作、PTY、导入/文件监听 | 每项有具体场景和独立验收 | D05、P06、W05 |

M1–M5 合起来是 P0，M2/M3 期间可以先使用简陋文本事件输出。CLI 与 UI 共用引擎，避免最后才把控制逻辑从 TUI 中拆出来。

后台运行若是首版刚需，在 M3 后增加 supervisor/IPC 阶段，再接入 M4 的 TUI；相应验收也提前。macOS 与 Windows 可以分开交付，不以三平台全部完成阻塞 Linux 首版。

## 2. 第一轮可直接执行的任务

1. 定义配置 v1 的 Rust 类型，明确可选字段、默认值、未知字段和错误位置。
2. 实现配置路径到项目目录、服务 cwd、env_files 的解析，保留原始值用于报错。
3. 建立 services 邻接表、校验环和依赖条件，生成确定性的启动计划。
4. 增加 `validate` 和 `plan [service...]` CLI；帮助里说明当前能力与尚未支持项。
5. 用临时目录测试跨目录项目、空格路径、缺失变量、ready 无探针、依赖环；不执行真实项目命令。
6. 根据实际 API 和 MSRV 添加必要依赖，再进入 M2 的真实进程测试。

M1 完成后，应能预览以下语义，不表示立刻同时启动三个进程：

```text
db   /srv/dev/infra        spawn → wait exec readiness
api  /srv/dev/reader-api   wait db.ready → spawn → wait exec readiness
web  /srv/dev/reader-web   wait api.ready → spawn
```

## 3. 开源项目调研结论

以下“项目事实”来自官方资料；“借鉴建议”是针对 devctl 的判断。此次是架构和需求层面调研，不宣称已经完整审计参考项目源码或验证其可靠性。动手学习时固定 tag/commit，避免跟随移动分支导致笔记失效。

### mprocs 与 dekit：操作体验和执行边界

项目事实：旧版 mprocs 支持多命令展示、进程交互、argv/shell、cwd/env 和停止策略；官方保留了旧版文档。[mprocs legacy README](https://github.com/pvolok/dekit/blob/master/README-mprocs.md)

当前官方仓库已更名 dekit；新版描述了依赖、崩溃/重启、CLI/TUI 和关闭终端后继续运行。因此不能把当前竞品能力概括成“mprocs 只有并行启动”。[dekit README](https://github.com/pvolok/dekit)

借鉴建议：学习进程选择、聚焦、滚动、快捷键、停止反馈；分别研究 pipe 与交互终端的需求。devctl 的差异应来自跨仓库工作区和清楚的恢复体验，而不是声称别人没有依赖管理。

源码阅读入口：当前目录包含 `src/config`、`src/process`、`src/runner`、`src/protocol`、`src/mprocs` 和终端相关模块，可按“配置 → runner → process → 交互/协议”追踪。目录划分只是阅读线索，具体实现要在固定版本中确认。[dekit src](https://github.com/pvolok/dekit/tree/master/src)

不建议初版照搬嵌入脚本语言、生产运行能力或完整终端系统，它们会扩大开发工作区工具的边界。

### Process Compose：依赖条件与故障语义

项目事实：官方项目将非容器应用的依赖、就绪/健康检查、恢复策略和 TUI/CLI 作为能力范围；生命周期文档介绍启动条件和恢复策略。[官方仓库](https://github.com/F1bonacc1/process-compose)、[Processes Lifetime](https://f1bonacc1.github.io/process-compose/launcher/)

借鉴建议：优先读“进程存活、启动就绪、一次性任务完成”之间的区别，再研究依赖失败如何显示、何时重试、下游是否重启。它比单纯的多进程展示更接近 devctl 的编排需求。

阅读顺序：官方生命周期说明 → readiness 配置 → 选择一个冷启动失败场景，追踪调度、探针取消和恢复预算。记录它的行为以及 devctl 是否采用，不照搬整个配置面。

首版不引入它的全部高级能力，例如远程 API、复杂模板、调度和多副本。

### Ratatui：界面结构与终端恢复

项目事实：Ratatui 是 TUI 库，官方提供 TEA、组件等应用组织方式；异步进程调度仍由应用自身负责。[Application Patterns](https://ratatui.rs/concepts/application-patterns/)、[FAQ](https://ratatui.rs/faq/)

借鉴建议：从最小事件循环、Model/Update/View、列表与日志布局开始，重点学习 resize、raw mode、退出恢复和 TestBackend。先用应用入口 crate，不因它内部拆分多个 crate 就提前拆 devctl。

学习成果应该是一个消费假状态快照的简单服务列表与日志面板，再把它连接到 EngineHandle。让 UI 代码不包含启动子进程和拓扑调度。

### Overmind：PTY 的收益与代价

项目事实：Overmind 基于 Procfile 和 tmux，并支持连接到服务进程及单服务重启。[Overmind README](https://github.com/DarthSim/overmind)

借鉴建议：理解终端运行与普通 pipe 在缓冲、颜色和交互方面的差异。devctl 先交付 pipe 日志，用户出现 stdin 交互需求后再评估 PTY；不让首版依赖 tmux。

### Tokio 与 Windows 官方 API：可靠性基础

重点核实三件事：Child drop 不等于停止、退出子进程需要 wait、广播队列会对慢消费者丢消息。平台管理参考 Unix 进程组 API 和 Windows Job Object 规则。[Tokio process](https://docs.rs/tokio/latest/tokio/process/index.html)、[Tokio broadcast](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)、[Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

这些 API 约束直接转化为 devctl 的测试：停止后代、回收 Child、处理迟到事件、日志洪水不影响控制，以及 Windows Job 关联竞态。

## 4. 学习方式

每次只选择一条流程，例如“用户重启 API”，记录：输入在哪里处理、谁拥有状态、怎样停止旧进程、怎样等待退出、何时启动新实例、错误怎样反馈。然后写出 devctl 自己的行为和可执行验收。

原型只针对不确定的系统边界：Unix 进程组停止、leader 先退出、exec 探针超时、日志洪水、Windows Job 关联。先确认行为，再做漂亮界面。

借鉴行为和结构时保留来源；复制代码时检查对应版本的许可并保留要求的声明。规划阶段无需 vendoring 或 fork 参考项目。

## 5. 发布前检查

- 文档、CLI 帮助与真实能力一致，明确平台支持范围和前台退出语义。
- 需求文档的 P0 验收场景全部具备结果，记录 Linux 和 WSL2 的环境。
- 默认不自动重启，启用重启后有预算；停止和就绪超时没有无限等待。
- 输出洪水下容量和界面响应达到已声明预算，所有丢弃有统计。
- 配置读取不执行命令，恢复不信任旧 PID、不保存环境值。
- 依赖只保留实际需要的库/features；记录构建尺寸和 MSRV，暂不为单二进制做复杂优化。

## 6. 规划文档验证记录

此次交付只新增规划文档，没有实现服务管理，也没有改动源代码、依赖、工作区配置或部署配置。文档中的 TOML 示例可做语法与引用检查，但不能当作通过了真实 devctl schema 校验或生命周期测试。

已完成检查：五份文档的本地链接和代码围栏完整；使用 Python `tomllib` 解析配置示例；核对项目/服务引用、ready 目标的探针和依赖图无环；确认现有入口、空依赖列表与工作区配置保持原样。交互补充后已核对项目/工作区范围与默认停止语义在需求、架构和交互文档中一致。未运行服务生命周期、平台或性能测试，因为对应实现尚不存在。
