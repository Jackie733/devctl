# 实施路线

本文只维护阶段、顺序与需求映射。需求和验收以 [requirements.md](requirements.md) 为准，当前小步与验证结果见 [tasks.md](tasks.md)。阶段完成必须有验收证据，不能由某个小步完成推断。

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
| M8 经需求验证的扩展 | 持续健康、级联重启、PTY、导入/文件监听 | 每项有具体场景和独立验收 | D05、P06、W05 |

M1–M5 合起来是 P0，M2/M3 期间可以先使用简陋文本事件输出。CLI 与 UI 共用引擎，避免最后才把控制逻辑从 TUI 中拆出来。

后台运行若是首版刚需，在 M3 后增加 supervisor/IPC 阶段，再接入 M4 的 TUI；相应验收也提前。macOS 与 Windows 可以分开交付，不以三平台全部完成阻塞 Linux 首版。

## 2. 推进规则

- 每次只从当前阶段拆一个可验收的小步，写入 [tasks.md](tasks.md)，不在此维护第二份详细任务清单。
- M1 尚未整体验收时可以先验证 M2 的进程风险，但不能把配置能力标记为全部完成。
- 发布依据 [requirements.md](requirements.md) 的首版验收；测试方法见 [DESIGN.md](../DESIGN.md) 的验证策略。
- 后续阶段只在需求或顺序变化时修改，不为每次代码提交更新本文件。

## 3. 按需学习入口

这些是阅读入口，不是依赖清单，也不要求提前研读全部源码。进入对应阶段时选定版本，沿一个具体行为追踪即可。

| 阶段 | 参考 | 学习重点 |
| --- | --- | --- |
| M2 | [Tokio process](https://docs.rs/tokio/latest/tokio/process/index.html) | Child 所有权、退出等待、停止与回收 |
| M3 | [Process Compose](https://github.com/F1bonacc1/process-compose) | 依赖就绪、失败阻塞与重试语义 |
| M4 | [dekit](https://github.com/pvolok/dekit)、[Ratatui](https://ratatui.rs/concepts/application-patterns/) | 进程操作、界面状态与渲染分离 |
| M7 | [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) | 后代归属与清理边界 |
| M8 | [Overmind](https://github.com/DarthSim/overmind) | PTY 与普通管道的行为差异 |
