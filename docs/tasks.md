# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 按服务 ID 登记任务并阻止重复启动：待誊写

- 阶段：[roadmap.md](roadmap.md) M3；需求：[requirements.md](requirements.md) P01、P02、P04；设计：[DESIGN.md](../DESIGN.md) §1、§3、§4。
- 目标：Manager 用 BTreeMap 持有每项 ServiceTask；同一 ID 的上一轮未验收清理前，拒绝创建新任务；按 ID 请求停止和等待结果。
- 改动：新增 manager.rs，提供 start、request_stop、wait；start 在创建任务前拒绝重复 ID 和不支持的依赖；wait 取得成功的运行结果后释放 ID；run_service 通过 Manager 托管单项。
- 验收：重复启动、已发送停止请求及取消等待后仍保留 ID；停止和自然退出完成清理后可重新使用 ID；未知 ID 报错；单项停止或失败不自动停止其他已登记项。
- 教学代码验证：`/tmp/devctl-manager-step.UbcJWE` 隔离副本中，宿主 `cargo test --locked --offline` 60 项通过（57 项基线 + 3 项登记器测试）；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。当前源码未应用此步，等待用户誊写后正式验收；隔离副本保留待复用的测试。
- 边界：当前错误为 String，无法区分未创建进程的启动失败与清理失败，因此任何 wait 错误暂时保留 ID。后续须明确错误分类以允许安全重试，不能按错误文本释放 ID。这里是管理组件，命令入口仍只托管一项；多项 CLI、全部停止、自动完成事件、运行锁和后台仍待实现。
- 下一步：验收登记器后区分安全释放与清理未知的错误，再推进多项完成处理与全部停止。

## 上一步验收：将每项运行封装为 ServiceTask

- 基线：`14a8634`；用户已完成 service_task.rs 和 main.rs 誊写。
- 审查：request_stop 取出发送端，重复调用不重复发送；wait 借用句柄，完成后才清空字段，重复等待报错；任务错误保留服务 ID；正常信号路径仍等待清理。未发现功能性誊写问题。
- 修复：统一任务错误信息的 Service 大小写，按仓库约定整理 main.rs 导入位置。
- 测试：复用上轮隔离副本的 2 项测试并正式落盘，覆盖双任务同时运行、单项停止、重复停止、重复等待、取消等待后继续清理和启动失败隔离；检查直接子进程实际回收。保留全部现有信号与后代清理测试。
- 验证：宿主 `cargo test --locked --offline` 57 项通过；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与文档本地链接检查通过。
- 限制：Linux /proc 可见性；清理超时、读取权限错误与任务 panic 未做故障注入；真实 npm/pnpm 项目仍待试用。ServiceTask 不提供服务 ID 互斥，停止请求和句柄 drop 也不代表清理成功。

## 必要交接

用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务运行；结束开发时显式全部停止，另有结束后台工作区的操作。三个命令无需等待中台就绪。

先完成多项目控制与后台，再接入 TUI。需求文档 §4 的其他扩展在 MVP 完成并试用后评估，不阻塞当前开发，不提前实现。测试通过不代替用户完整流程验收。
