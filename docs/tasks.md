# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：同步目标安装，准备 M6 单项启动验收

- 本轮起点 9689070，工作区干净。用户提供了三个真实项目的脚本、工具链与配置检查输出；助手直接实现与验证，用户最后审核。
- 关联 M6、W01、W03、P01、P02、X01。本轮只记录目标环境证据并明确下一步，未改功能代码或项目依赖。
- 目标环境：Windows WSL2、Ubuntu 22.04.5 LTS、x86_64。已确认启动路径 /home/jackie/.cargo/bin/devctl；本轮版本输出仍是 0.1.0，仓库为 0.2.0。e314939 帮助框效果尚待目标安装后确认。

## 已收到的目标检查结果

- Node v24.18.0、npm 11.16.0；node/npm/pnpm 均位于 fnm 的 v24.18.0 安装目录。尚未提供 pnpm 的版本。
- web 的 dev = vite；weapp 的 dev:weapp = npm run build:weapp -- --watch，build:weapp = taro build --type weapp；middle 的 dev = egg-bin dev。展示的 scripts 中未见对应 pre/post 启动钩子或 pnpm install。
- middle 声明 engines.node >=18.0.0，当前 Node 满足该声明；web、weapp 的输出未显示 engines 或 packageManager。尚未用实际依赖版本和项目运行结果确认兼容性。
- 指定 ~/.config/devctl/devctl.toml 的 validate 成功，配置包含三个已确认目录及三个服务；plan 中 middle/web 为 npm run dev，weapp 为 npm run dev:weapp，三者 autostart 均为 true。以上是当前 0.1.0 的磁盘配置检查结果，不能证明现存后台已经加载这些配置。
- 此前实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据，不能据本轮脚本输出认定原因或已修复。依赖安装情况、实际编译与 stdin 需求仍待逐项运行确认。

## 验证与必要交接

- e314939 帮助框调整已完成本地全量 182 项测试（含 7 项真实 PTY）、cargo fmt --check 和 cargo clippy --locked --offline --all-targets -- -D warnings；该验证不代表目标 WSL 与真实项目验收通过。
- 9689070 已提供只读检查说明。本轮检查文档本地链接、代码围栏及 git diff --check 通过；纯任务记录变更，未重跑 Rust 测试。
- 已提供 /tmp/devctl-e314939.tar.gz 源码包，功能代码与当前仓库一致。尚未在用户 WSL 安装最新构建，未新增或自动推送发布产物。

## 下一步

1. 使用包含 e314939 的源码，在目标 WSL 方便停止服务时结束旧工作区，再重新安装。安装方式见 [wsl.md](wsl.md) §2、§5。确认 /home/jackie/.cargo/bin/devctl --version 为 0.2.0，Zsh rehash 后核对路径；检查帮助框按内容收缩的效果。q 或 stop --all 不会结束旧后台或重载配置与环境。
2. 从全部停止开始进入固定配置的 TUI，只启动 web，确认 weapp/middle 仍停止；检查 Vite 日志与实际页面，失败时保留完整错误，不据进程存活判断业务就绪。
3. 按 [requirements.md](requirements.md) §3 继续全部启动且 web 不重复创建、单项停止再启动、离开/关闭终端与重连、全部停止、结束工作区，核对后代清理及端口释放；遇到项目依赖问题先按项目约定处理。
4. 核对日志停止状态与保留输出、old 标记，以及 V → j/k → y 的目标终端粘贴体验；记录用户结论并修复验收发现的问题。

仅提交本步，不自动推送。MVP 完整验收前不推进 X03；公开发布与 devctl update 的计划见 [requirements.md](requirements.md) §4.1。
