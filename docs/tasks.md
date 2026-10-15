# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：WSL 截图仍为旧版 TUI，待核对实际启动文件

- 本轮起点 cd2b276，工作区干净。用户截图底部仍有三行，快捷键文本及 Logs: web | FOLLOW 标题与 b5cc1e1 的旧布局一致；当前两行布局已在 143dce2 实现，日志状态分离在 054665a 实现。关联 M5、M6、U01、L01、X01。
- 当前代码底部高度为一行状态加一行快捷键；本轮未发现需要修改视图的证据。已请求用户提供 WSL 源码的 git log -5 --oneline 与 whence -va devctl，尚不能确定是源码未同步、运行中的旧客户端、安装路径或别名/函数问题。
- 在 [wsl.md](wsl.md) §5 增加核对客户端、源码与双安装路径的排查步骤。布局由客户端绘制，单独重建后台不能更新旧 TUI。
- 公开发布与 devctl update 的 X03 已确认在 MVP 后实施，范围见 [requirements.md](requirements.md) §4.1，阶段顺序见 [roadmap.md](roadmap.md) §4；当前主线仍为 WSL 真实项目验收。

## 本轮验证

- cargo test --locked --offline --bin devctl tui::view::tests：10 项通过，包含底部固定两行、宽窄屏、日志状态与保留输出隔离。对照 b5cc1e1 的 git show，截图的两行快捷键文本与旧版一致。
- Python 3 检查修改文档的本地链接和代码围栏；git diff --check 通过，已审查差异。本轮仅修改文档，未重跑完整测试、fmt 或 clippy。
- 当前验证不能证明 WSL 实际启动文件已更新，需结合用户命令输出继续定位。

## 必要交接与下一步

- 优先核对 WSL 命令输出；若源码已更新，可用 wsl.md 中的显式 cargo run 绕过安装路径验证，再修正实际安装。不要仅凭同为 0.1.0 的 --version 判断更新成功。
- cd2b276 已将 Zsh 作为安装主示例，并通过含实际 Zsh 默认/自定义 ZDOTDIR、新交互/登录终端的 10 项脚本检查；目标 WSL 的真实环境仍待用户核对。
- 8831bd5 已实现 PATH 设置与外部更新器；上一轮 9 项隔离脚本测试、真实 Linux 二进制安装/版本/配置校验及 fmt、clippy 通过。HTTPS 下载使用替身，未连接真实发布服务；目标 WSL 的完整更新流程仍待试用。
- 按 [wsl.md](wsl.md) 同步脚本至 WSL，一次性设置 PATH 并安装更新器；目前需要可信构建方提供兼容的二进制和校验值，没有正式发布附件。未自动推送，远程拉取不一定包含本地提交。
- 在方便重启时结束各工作区，试用更新和回退，核对 command -v devctl 指向 ~/.local/bin/devctl，配置保持原样。首次从 Cargo 迁移没有 previous 备份，旧 Cargo 安装仍可通过其绝对路径使用。
- 054665a 的日志状态与保留输出、old 标记仍待真实项目审核；V → j/k → y 的目标终端粘贴体验及三个真实项目的 WSL2 M6 完整流程继续保留。
- web 为 npm run dev；旧后台沿用创建时配置与环境。实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据，发行版、架构、实际脚本/依赖和 stdin 需求仍待核实。
- 助手直接实现与验证，用户最后审核；仅提交本步，不自动推送，MVP 验收完成前不推进 X03 实现。
