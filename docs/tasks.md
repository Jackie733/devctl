# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## TERM 宽限与超时强杀：待誊写

- 阶段：[roadmap.md](roadmap.md) M2；需求：[requirements.md](requirements.md) P04；设计：[DESIGN.md](../DESIGN.md) §4。
- 目标：停止时先向组发 TERM，给 leader 最多 5 秒退出；leader 退出或超时后强杀残留组，最后 wait 直接子进程。
- 设计：Linux waitid 的 WNOWAIT/WNOHANG 观察退出而不回收，保留 leader PID 到最后一次组信号；Tokio time 提供非阻塞等待与超时。
- 本步验收：TERM 清理逻辑确实执行；忽略 TERM 时约 5 秒强杀；leader 先退出时仍强杀残留 worker；原有进程测试继续通过。
- 验证：隔离临时目录编译与 Clippy 检查通过；宿主原型验证正常 TERM（约 0.03 秒）、忽略 TERM（约 5.04 秒）、leader 先退出且 worker 忽略 TERM（约 0.03 秒）三种情况。功能代码未写入当前源码，等待用户誊写后正式验收。
- 边界：本步沿 Linux 方向；leader 的自然退出路径清理、强杀后整个进程组完成确认、错误路径与停止竞态仍须后续验收，不代表 P04 全部完成。
- 下一步：用户誊写后审查、补齐本步行为测试，再继续完善组清理。

## 上一步验收：进程组强制停止

- 基线：`bb0a791` 的直接子进程停止；用户已添加 nix signal feature 和 killpg + Child wait。
- 审查：本轮 Child PID 对应独立进程组；检查整数转换、ESRCH 和其他错误处理；将 wait 失败提示改为明确的回收错误。
- 测试：新增 shell 派生 sleep worker 的真实进程树场景，确认两者终止、直接 Child 被回收、输出管道关闭；复用 SIGINT/SIGTERM 和原有进程测试，失败清理改为覆盖整组，所有等待有期限。
- 验证：宿主 `cargo test --locked --offline` 43 项通过；最后错误提示调整后 `cargo test --locked --offline --test process_cli` 12 项通过；fmt、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。
- 限制：Linux worker 检查区分已终止的 zombie 与运行进程，不宣称 devctl 回收非直接后代；未验收优雅停止或后台行为。信号行为沿用宿主环境验证，受限沙箱此前存在超时。

## 最近交接（2026-10-10）

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令仍无需等待中台就绪。

后台已纳入首版，路线改为先实现后台控制，再接入 TUI；当前 M2 审查继续支撑可靠停止。用户已确认需求文档 §4 的其他扩展在 MVP 完成并试用后再评估；不阻塞当前开发，不提前实现。

本轮验收用户誊写的进程组强杀并提交必要测试；下一步 TERM 宽限与超时强杀由用户学习并誊写。
