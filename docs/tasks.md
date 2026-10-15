# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：TUI 顶栏显示客户端版本，继续 M6 验收

- 本轮起点 25a8681，工作区干净。用户反馈帮助弹窗“合适”、单项试用“正常”，并要求 TUI 显示版本号；助手直接实现与验证，用户在目标 WSL 试用。
- 关联 M5、M6、U01、X01。顶栏在 devctl 后显示客户端包版本，自动取自 Cargo 包元数据，与 --version 一致；显示规则见 [interaction.md](interaction.md) §2.4。版本仍为 0.2.0，不增加后台协议字段。
- 目标环境：Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；此前确认路径 /home/jackie/.cargo/bin/devctl。用户已确认安装输出 0.2.0；本轮新增的顶栏版本信息尚待重新安装后查看。

## 必要的目标项目交接

- Node v24.18.0、npm 11.16.0；node/npm/pnpm 均位于 fnm 的 v24.18.0 安装目录。
- web 的 dev = vite；weapp 的 dev:weapp = npm run build:weapp -- --watch，build:weapp = taro build --type weapp；middle 的 dev = egg-bin dev。展示的 scripts 中未见对应 pre/post 启动钩子或 pnpm install。
- middle 声明 engines.node >=18.0.0，当前 Node 满足该声明；web、weapp 未显示 engines 或 packageManager。小程序、中台的实际依赖兼容性与 stdin 需求仍待运行确认。
- 用户此前用 0.1.0 执行固定 ~/.config/devctl/devctl.toml 的 validate 成功；plan 包含三个正确目录，middle/web 为 npm run dev，weapp 为 npm run dev:weapp，三者 autostart 均为 true。旧后台保留创建时配置与环境，更新前处理方式见 [wsl.md](wsl.md) §5。
- e314939 源码包已用于目标安装；本轮用户确认帮助框与 web 单项试用正常，未提供逐项实例记录。此前实例 #6 的 pnpm install 错误没有新复现证据，不据此认定原因；后续若复现，保留完整日志。

## 本轮验证

- cargo fmt --check：通过。
- cargo clippy --locked --offline --all-targets -- -D warnings：通过。
- cargo test --locked --offline tui::：35 项 TUI 单元测试通过；在既有布局检查中核对 120×30、80×12、60×16、36×10 的版本显示，与包版本一致。
- cargo test --locked --offline --test tui_cli --test cli：在允许本地 Unix socket 的沙箱外运行，14 项通过，含 7 项真实 PTY 测试。
- 本轮检查修改文档的本地链接、代码围栏及 git diff --check 通过。仅改顶栏显示，未重跑其他进程管理回归；未在用户 WSL 重新安装或确认新顶栏效果。

## 下一步

1. 将包含本轮顶栏改动的源码同步到目标 WSL 并重新安装，确认 TUI 显示 devctl 0.2.0；之前的 e314939 包不包含本轮版本显示。
2. web 单项试用正常后，按 a 启动全部，检查三个项目日志与实际开发状态，并核对 web 没有重复创建；再单独停止 web，确认另两项继续运行，重新启动 web。
3. 按 [requirements.md](requirements.md) §3 继续离开/关闭终端与重连、全部停止、结束工作区，核对后代清理及端口释放；补齐失败反馈、日志停止状态与保留输出、old 标记，以及 V → j/k → y 的目标终端粘贴体验。

仅提交本步，不自动推送。完整真实项目验收仍未完成；MVP 完整验收前不推进 X03，计划见 [requirements.md](requirements.md) §4.1。
