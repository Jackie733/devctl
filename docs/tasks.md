# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 已完成本轮范围：后台控制与有界日志，停在 TUI 前

- 用户授权助手直接完成 TUI 之前的编码、测试和逐步提交，本轮范围已完成。阶段：[roadmap.md](roadmap.md) M3/M4；需求：[requirements.md](requirements.md) P01、P07、P02、P04、P05、R02、L01、L03；机制：[DESIGN.md](../DESIGN.md) §3–6。
- 已有提交：5307365 身份探测；4f27fb1 独立后台及控制 CLI。本轮最后一组为有界日志、运行实例/后台实例标识和交接；最终提交见 Git。
- 可用入口：start 单项/多项/--all 按需创建后台；status、logs、stop、shutdown 只连接已有后台。start 回报受理；stop 等待清理完成，stop --all 保留后台；shutdown 安全收尾后结束后台。前台 run、validate、plan 保留。
- 后台：setsid 与标准流重定向；私有启动报告、有界实际身份握手、并发创建连接胜者；连接现存实例不重新加载配置或环境。网络任务和日志读取任务不拥有服务，客户端断连不撤销已受理操作。
- 日志：后台持续读取 stdout/stderr，有界队列与每服务近期缓存；按服务、运行实例、来源和序号显示，尾部/游标分页读取；截断、丢弃、缓存缺口和读取不完整均有提示。后台重建改变 workspace_id，供后续 TUI 清除旧游标/状态。
- 安全审查：全部停止通过完成事件推进，清理期间仍可查询状态；停止全部覆盖手动项，阻止新启动。清理未知时保留错误、登记、后台、端点及锁，不虚报退出；不根据旧 PID 恢复或终止进程。accept 错误定时退避，不阻塞其他事件或丢失所有权。
- 实际验证：宿主 `cargo test --locked --offline` **129 项通过**；`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与改动文档本地链接检查通过。
- 受控验收覆盖：真实 PTY 关闭与重连、标准流指向 /dev/null、8 客户端并发创建不重复、单项/全部启停及重启、失败隔离、配置不重载、停止期间状态响应、请求断连后继续清理、启动错误报告、后台实例重建、日志游标/来源/轮次、超长/坏编码/控制字符、持续双流高输出、队列和字节/记录数上限、读取错误与排空超时。原有配置、前台生命周期与锁测试全部保留。
- 环境：Unix socket/信号测试需在允许本地 socket 与进程控制的宿主运行；本会话沙箱曾返回 bind Operation not permitted，先前信号超时根因未定位。宿主测试使用 Linux、Rust 1.98.1、sh/sleep/Python 3；没有宣称其他平台或 Rust 最低版本实测通过。
- 限制：清理未知的管理边界通过错误注入及代码审查验证，未制造真实内核清理超时或 /proc 权限故障；不做 SIGKILL/断电后的孤儿恢复。硬链接配置别名和网络文件系统互斥不在已验证范围；配置目录需可写，过长 Unix socket 路径明确报错。测试通过不代替真实项目试用。

## 当前任务：M5 第一小步，终端建立与恢复（等待誊写）

- 用户已取消 black-prism 启动动画；撤下预览工具、项目内生成资源与相关设计，后续进入 TUI 直接显示管理界面。动画探索历史由 Git 保留，不作为待验收或下一步任务。
- 正式 TUI 尚未编码，未添加终端依赖；技术草案见 [DESIGN.md](../DESIGN.md) §5.1，交互规则见 [interaction.md](interaction.md)。主线仍为 U01、P01、P07、P02、P05、L01、L03。
- 上轮设计交接：78dc5ad，确认左右分栏，补充模块、状态归属、请求并发和验收顺序；文档差异与本地链接检查通过。用户现在要求进入第一小步，继续由用户学习和誊写功能源码。
- 本步对应 U01：新增显式 tui 命令、输入/输出 TTY 检查、最小左右分栏及窄窗口提示；处理 q、raw mode Ctrl-C、SIGINT/SIGTERM/SIGHUP、窗口变化和终端恢复。仅展示未连接占位，不加载配置、不连接或创建后台，不显示模拟项目状态。
- 教学代码先只建立 src/tui/mod.rs 和 terminal.rs；有真实界面状态后再引入 App 和独立 view。终端 guard 在修改终端前创建，记录尝试过的设置（写入可能部分失败），错误路径回滚；恢复各项独立尝试并保留错误，Drop 与 panic hook 兜底。
- 状态区分：功能代码、测试夹具和更新的 Cargo.lock 仅在 /tmp/devctl-tui-step1-ta02nj4m 临时副本；主仓库源码和依赖未改、未誊写、未提交。临时锁定版本为 Ratatui 0.30.2、Crossterm 0.29.0、futures-util 0.3.34；futures-util 仅 std，Ratatui 仅 std/crossterm，Tokio feature 保留。临时缓存位于该目录 cargo-home，避免写入只读的用户 Cargo 缓存。
- 接线前缺口：client 会把 Response.error 转成字符串并丢失后台身份；部分后台错误本身表示完成未知，不能统一当成拒绝。建议增加保留 Response 的事务入口，CLI 包装保持兼容；不解析错误文案或自动重放操作。
- 临时副本验证：CARGO_HOME 指向上述临时缓存，`cargo check --offline`、`cargo build --locked --offline`、`cargo test --locked --offline --test cli`（7 项，其中新增无 TTY 检查并扩充 tui 帮助检查）、`cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings` 通过。未重跑后台完整测试。
- 宿主 `python3 -u check_pty.py`：8 项通过，覆盖 q、按键 Ctrl-C、外部 SIGINT/SIGTERM/SIGHUP、窄/宽窗口切换、同一 guard 的 panic hook 和错误返回 Drop；核对退出码、终端模式、备用屏幕和光标。沙箱外部信号仍出现退出超时，宿主未复现；信号验收以宿主结果为准。设置中途失败的回滚仅代码审查，未注入真实终端初始化/恢复失败；未声称实际项目流程通过。
- 主仓库本轮文档验证：`git diff --check` 通过；Python 本地链接检查通过（2 份改动文档、11 个本地链接）；差异仅为设计验证状态与当前任务交接。

## 下一步：用户誊写本步，然后审查验收

- 用户誊写依赖、Tui 子命令、main 接线与两个新文件，运行 build、fmt、clippy 及真实终端自检。下一轮先核对实际源码与功能目标，补充有价值的测试、验证并提交本步，之后才提供后台状态列表代码。
- 教学小步：终端建立/恢复（U01）→ 空闲后台与状态列表（U01/P05）→ 单项/全部控制（P01/P07/P02）→ 有界日志与滚动（L01/L03）→ 重连、结束工作区及 PTY 完整验收（P05/U01）。每步保留相关错误路径验证。
- 建议 Ratatui + Crossterm EventStream + 现有 Tokio；App 独占 UI 状态，网络任务不阻塞输入，沿用单事务 IPC。client 的字符串错误接口需在接线时区分后台响应与本地失败，保留工作区身份。
- TUI 复用 control 请求/响应，接线时使用保留完整 Response 的 client 事务入口。初次查询可按需创建空闲后台，但不能隐式发送 Start；状态与日志异步获取，绘制不进入后台管理循环。
- q/Ctrl-C 离开只恢复终端并断开，服务保留；结束工作区是独立 shutdown。断连显示未知，重连重新取状态和近期日志；workspace_id 改变时丢弃旧缓存，分页取最后实际收到的日志序号，不盲目重放控制操作。
- 用户确认的真实场景为三个独立项目、每项目一个命令，无需依赖就绪。真实目录、平台及 stdin 交互需求仍待试用核实；M6 再完成 npm/pnpm 的全流程验收。需求 §4 的扩展不提前实现。
