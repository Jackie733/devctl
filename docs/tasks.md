# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：修正 Zsh 安装指引，继续 WSL 试用

- 本轮起点 07395c2，工作区干净。用户确认交互 shell 为 Zsh，原主示例的 bash 目标会写入错误的启动文件；关联 M6、X01，修正 [wsl.md](wsl.md) §2 为明确的 Zsh 命令。
- 设置脚本仍由 Bash 解释，目标参数使用 zsh；从调用方显式传递 ZDOTDIR，使用 rehash 刷新当前终端缓存。保留错误写入 .bashrc 后的修正说明；已有脚本已支持 Zsh，本轮无需修改脚本或 Rust 源码。
- 公开发布与 devctl update 的 X03 已确认在 MVP 后实施，范围见 [requirements.md](requirements.md) §4.1，阶段顺序见 [roadmap.md](roadmap.md) §4；当前主线仍为 WSL 真实项目验收。

## 本轮验证

- python3 tests/support/install_scripts.py：10 项通过。新增实际 Zsh 5.9 检查文档命令可在当前终端生效，新交互/登录终端可加载配置；覆盖默认目录和未导出的自定义 ZDOTDIR，确认没有误写 .bashrc。全部使用临时 HOME，未修改真实用户环境。
- Python 3 检查修改文档的本地链接和代码围栏；git diff --check 通过，已审查差异。仅修改文档与检查脚本，未运行 Rust 测试、fmt 或 clippy。
- 上述为当前 Linux 宿主验证，目标 WSL 的真实启动文件、插件加载及新终端体验仍待用户核对。

## 必要交接与下一步

- 8831bd5 已实现 PATH 设置与外部更新器；上一轮 9 项隔离脚本测试、真实 Linux 二进制安装/版本/配置校验及 fmt、clippy 通过。HTTPS 下载使用替身，未连接真实发布服务；目标 WSL 的完整更新流程仍待试用。
- 按 [wsl.md](wsl.md) 同步脚本至 WSL，一次性设置 PATH 并安装更新器；目前需要可信构建方提供兼容的二进制和校验值，没有正式发布附件。未自动推送，远程拉取不一定包含本地提交。
- 在方便重启时结束各工作区，试用更新和回退，核对 command -v devctl 指向 ~/.local/bin/devctl，配置保持原样。首次从 Cargo 迁移没有 previous 备份，旧 Cargo 安装仍可通过其绝对路径使用。
- 054665a 的日志状态与保留输出、old 标记仍待真实项目审核；V → j/k → y 的目标终端粘贴体验及三个真实项目的 WSL2 M6 完整流程继续保留。
- web 为 npm run dev；旧后台沿用创建时配置与环境。实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据，发行版、架构、实际脚本/依赖和 stdin 需求仍待核实。
- 助手直接实现与验证，用户最后审核；仅提交本步，不自动推送，MVP 验收完成前不推进 X03 实现。
