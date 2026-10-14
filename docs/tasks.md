# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 版本化身份探测与异步连接处理：待誊写

- 阶段：[roadmap.md](roadmap.md) M4；需求：[requirements.md](requirements.md) P05、R02；协议、资源预算与取舍：[DESIGN.md](../DESIGN.md) §6。
- 本步目标：将控制 socket 注册到 Tokio，接受最小版本化探测请求并返回规范化配置身份；有界并发和读写期限防止慢客户端阻塞其他探测及退出信号。不在本步接入服务启停、状态查询或后台启动器。
- 功能代码仅在教学副本 `/tmp/devctl-probe-step.ahlap5w5`，未应用到项目，等待用户誊写。副本使用独立 target，宿主 `cargo test --manifest-path /tmp/devctl-probe-step.ahlap5w5/Cargo.toml --target-dir /tmp/devctl-probe-step.ahlap5w5/target --locked --offline` 106 项通过（98 项基线 + 5 项探测测试 + 3 项集成测试）；副本 cargo fmt、Clippy 的既有检查命令通过。
- 教学验证覆盖：分段请求、原始非 UTF-8 身份、版本不兼容、请求不完整和断连、静默客户端超时、身份长度上限；配置别名返回规范身份、坏客户端不影响其他探测、满负载超时释放容量、静默连接不拖延信号退出。
- 依赖准备：下一步在 Cargo.toml 为 Tokio 启用 net、io-util，需要 Cargo.lock 增加 socket2（副本解析为 0.6.5）。该包已从本机缓存离线解压验证；项目 Cargo.toml/Cargo.lock 尚未应用此改动，誊写后先 cargo check --offline 更新锁文件，再运行 --locked 检查。
- 现有端点组件测试已使用 Tokio 测试上下文，为下一步注册 I/O 驱动作准备；用户不需改写测试。新探测及集成测试仍保存在副本，下轮验收时正式复用。
- 下一步验收后继续进程脱离终端和有界启动握手，利用真实探测核对工作区身份；再接实例发现与控制 CLI。服务停止仍用 request_stop_all + wait_next 推进，保持控制循环可响应。

## 上一步验收：本地控制端点绑定与清理

- 基线：`6a82ef7`。用户明确授权助手完成本步誊写，已应用 control_socket.rs、supervisor 接线、模块声明、nix user feature 与生成目录忽略规则；五项组件及两项集成测试已正式落盘。
- 审查：先取得同一规范化配置的锁，再绑定端点；私有目录检查、不覆盖其他文件、残留 socket 替换、guard 借用锁、按本次身份尽力清理均符合上轮教学约定，功能无需修复。尚未接受或处理请求。
- 验证：宿主 cargo test --locked --offline 全部 98 项通过；端点组件改为 Tokio 测试上下文后，宿主重跑 5 项组件测试通过。cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings 通过。
- 环境：本轮沙箱组件测试因 Unix socket bind 返回 Operation not permitted 失败；宿主复验通过。此前沙箱信号超时原因仍未定位，不把这些沙箱失败记为通过。
- 边界：端点目录由当前有效用户拥有且权限为 0700；权限检查不防御同一用户主动替换目录。配置目录需可写，路径过长明确报错。退出清理是尽力执行，崩溃残留端点由下一次持锁者处理，不代表残留服务已经清理或恢复。
- 提交前检查：差异与两份改动文档本地链接目标通过；无关 Rust 学习文档未纳入。

## 必要交接

- 运行锁与多项目控制核心已提交 `f17120c`，空闲管理入口已提交 `6a82ef7`。清理未知保留登记和原始错误，禁止该项再次启动；不根据旧 PID 发信号或自动恢复。
- 用户确认必须支持：任意一个项目单独启动、全部启动、TUI、项目日志、后台运行。退出界面默认保留服务；结束开发时显式全部停止，另有结束后台工作区操作。三个命令无需等待中台就绪。
- 先完成多项目控制与后台，再接入 TUI。需求 §4 扩展在 MVP 完成并试用后评估。当前尚无 detached 后台、控制 CLI 或完整产品流程；测试通过不代替用户试用验收。
