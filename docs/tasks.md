# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：默认执行 devctl 进入 TUI

- 本轮起点 380f26a，工作区干净。用户要求省略 tui 子命令直接进入管理界面，显式其他子命令仍执行各自操作；关联 M5、M6、U01、W02、P05。
- 入口仅将缺省子命令映射到已有 Command::Tui，复用原配置发现、连接与终端流程。保留显式 tui；帮助与版本仍由 Clap 处理。操作规则统一见 [interaction.md](interaction.md) §1。
- 默认入口需要有效配置和终端输入/输出，缺配置或无 TTY 明确报错；未知子命令保持参数错误，进入界面仍不自动启动项目。

## 验证证据

- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings：通过。
- cargo test --locked --offline --test cli --test config_discovery_cli：15 项通过；覆盖帮助/版本不读配置、裸命令缺配置、默认与显式 TUI 拒绝非终端输入/输出、未知命令报错及配置优先级。
- cargo test --locked --offline：在允许 Unix socket 的沙箱外完整回归 186 项通过（109 单元、77 集成），含 8 项真实 PTY 测试。PTY 默认省略 tui，覆盖显式配置及两个 HOME 默认位置；其中一次重连显式使用 tui，确认与默认入口连接原后台且不重复启动。
- 修改文档的本地链接、章节锚点、代码围栏、表格与 Python AST 检查及 git diff --check 通过，已审查差异。未构建 release 产物，目标 WSL 尚待安装及默认入口试用。

## 必要交接与实际进度

- 目标为 Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；此前确认 /home/jackie/.cargo/bin/devctl 安装版本 0.2.0，Node v24.18.0、npm 11.16.0。
- 三项目配置与命令核对、帮助框尺寸、web 单项试用、全部启动与重复启动保护、单独停止 web 不影响另两项、web 再启动及 old 标记已由用户确认通过。
- 用户已确认 WSL clip.exe 的命令测试可在 Windows 粘贴；新 TUI 的原生复制、中间行选择、小写 v、顶栏版本号尚待明确反馈。最新构建将包含这些已有改动和本轮默认入口，版本仍为 0.2.0。
- 助手无法反向 SSH 连接目标，沿用 WSL scp 拉取源码包的方式；旧 e57c8c0 包不包含默认入口改动。
- 当前日志仍为近期内存缓存，历史分页、持久化与检索聚合已确认在 MVP 后实施，计划见 [requirements.md](requirements.md) §4.2，方案见 [DESIGN.md](../DESIGN.md#53-mvp-后日志增强方案尚未实现)；本轮不提前实现。

## 下一步

1. 同步本轮功能包并在目标 WSL 重新安装，从实际使用目录运行 devctl 或固定配置的 devctl --config PATH，确认直接进入界面、项目不自动启动；--help / --version 和显式控制命令仍可独立使用。
2. 用 j/k → v → j/k → y 复测原生复制，核对 via clip.exe、Windows 粘贴和顶栏版本显示。
3. 补齐 q 离开/关闭客户端终端后的原工作区与实例重连，随后验证 X 全部停止、重新启动无端口残留、Q 确认结束后台及失败反馈，完成 M6。

仅提交本步，不自动推送。MVP 完整验收后再安排 X03 与日志增强，当前验收依据仍为 [requirements.md](requirements.md) §2–§3。
