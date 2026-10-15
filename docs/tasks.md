# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：WSL PATH 持久化与外部更新器，待目标环境试用

- 起点 054665a，工作区干净。用户要求具体实现 PATH 持久化与外部更新脚本；关联 M6、X01，沿用 W02 已有配置发现行为，不新增 Rust 子命令或自动发布配置。
- [setup-path.sh](../scripts/setup-path.sh) 为目标 WSL 的 Bash / Zsh 追加一次 PATH，优先使用 ~/.local/bin，其次 ~/.cargo/bin；保留现有启动文件。脚本需由用户在 WSL 执行，本机会话未修改真实 shell 配置。
- [devctl-update](../scripts/devctl-update) 支持本地 Linux 二进制或 HTTPS URL + 明确的 SHA-256。暂存、核对后限时检查 --version，通过后同文件系统替换，保留一个 previous 文件并支持 --rollback；重复安装相同内容不覆盖备份，拒绝并发更新和覆盖自定义符号链接。
- 更新器不修改工作区配置、不自动结束后台或重启项目。用户需在切换或回退前逐一结束相关后台并退出旧 TUI；具体安装、准备二进制、切换与回退步骤统一维护在 [wsl.md](wsl.md) §2、§5。
- 当前没有正式发布附件或自动构建流程，需要先从可信构建方取得与目标架构、系统库兼容的二进制及校验值。更新器本身不消除发布方的编译工作；目标 WSL 可直接安装预编译文件。

## 本轮验证

- python3 tests/support/install_scripts.py：9 项通过。临时 HOME 覆盖安装、连续更新、重复安装、回退、配置保留、错误校验值不执行候选程序、候选程序失败、无备份/无效参数/HTTP、并发互斥、自定义符号链接、模拟下载成功/失败及 Bash PATH 优先级、Zsh 自定义启动目录。
- bash -n scripts/setup-path.sh scripts/devctl-update、cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings 均通过。未改 Rust 源码，未重跑全量 Rust 测试。
- 使用现有 target/debug/devctl 在临时 HOME 安装，--version 和显式指定 examples/process.toml 的 validate 成功；未启动真实项目或停止现存后台。
- 两份修改文档的 10 个本地链接与代码围栏检查通过，git diff --check 通过，已审查脚本、测试及文档差异。
- HTTPS 路径使用受控 curl 替身验证调用参数与失败保留行为，未连接真实发布服务器；Zsh 启动文件写入已测，未实际启动 Zsh。目标 WSL 的新终端加载、架构/系统库兼容性和完整更新流程仍需用户试用。

## 必要交接与下一步

- 同步本步脚本至 WSL，按 wsl.md 一次性设置 PATH 并安装更新器。没有自动推送，远程拉取不一定包含当前本地提交；先以实际同步文件为准。
- 在方便重启时结束工作区，用可信预编译文件试用更新和回退，核对 command -v devctl 指向 ~/.local/bin/devctl，配置保持原样。首次从 Cargo 迁移没有 previous 备份，旧 Cargo 安装仍可通过其绝对路径使用。
- 上一轮 054665a 已完成日志面板 STOPPED / Retained logs 与 old 标记实现及受控验证；真实项目停止后的状态辨识、V → j/k → y 的目标终端粘贴体验仍待用户审核，继续保留三个真实项目的 WSL2 M6 验收。
- web 为 npm run dev；旧后台沿用创建时配置与环境。实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据，发行版、架构、实际脚本/依赖和 stdin 需求仍待核实。
- 助手直接实现与验证，用户最后审核；仅提交本步，不自动推送，不提前实现其他候选功能。
