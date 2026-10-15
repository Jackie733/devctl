# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：M6 全部启动与单项停止验收

- 本轮起点 fca30e4，工作区干净。用户要求继续剩余任务；帮助框与 web 单项试用此前已反馈正常，当前进入全部启动、重复启动保护和单项停止再启动的目标 WSL 验收。
- 当前关联 M6、P01、P07、P02、L01、X01。助手维护实现与记录，用户在目标 WSL 操作并反馈。已有功能不再重复开发；若真实试用发现问题，先定位并做最小修复。
- 目标环境：Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；此前确认路径 /home/jackie/.cargo/bin/devctl。用户已确认安装输出 0.2.0；fca30e4 新增的顶栏版本信息尚待目标显示确认。

## 必要的目标项目交接

- Node v24.18.0、npm 11.16.0；node/npm/pnpm 均位于 fnm 的 v24.18.0 安装目录。
- web 的 dev = vite；weapp 的 dev:weapp = npm run build:weapp -- --watch，build:weapp = taro build --type weapp；middle 的 dev = egg-bin dev。展示的 scripts 中未见对应 pre/post 启动钩子或 pnpm install。
- middle 声明 engines.node >=18.0.0，当前 Node 满足该声明；web、weapp 未显示 engines 或 packageManager。小程序、中台的实际依赖兼容性与 stdin 需求仍待运行确认。
- 用户此前用 0.1.0 执行固定 ~/.config/devctl/devctl.toml 的 validate 成功；plan 包含三个正确目录，middle/web 为 npm run dev，weapp 为 npm run dev:weapp，三者 autostart 均为 true。旧后台保留创建时配置与环境，更新前处理方式见 [wsl.md](wsl.md) §5。
- e314939 源码包已用于目标安装；本轮用户确认帮助框与 web 单项试用正常，未提供逐项实例记录。此前实例 #6 的 pnpm install 错误没有新复现证据，不据此认定原因；后续若复现，保留完整日志。

## 上一步验证（fca30e4）

- cargo fmt --check：通过。
- cargo clippy --locked --offline --all-targets -- -D warnings：通过。
- cargo test --locked --offline tui::：35 项 TUI 单元测试通过；在既有布局检查中核对 120×30、80×12、60×16、36×10 的版本显示，与包版本一致。
- cargo test --locked --offline --test tui_cli --test cli：在允许本地 Unix socket 的沙箱外运行，14 项通过，含 7 项真实 PTY 测试。
- 本轮检查修改文档的本地链接、代码围栏及 git diff --check 通过。仅改顶栏显示，未重跑其他进程管理回归；未在用户 WSL 重新安装或确认新顶栏效果。

## 本轮检查

- 已审查任务记录、Git 状态、需求 §3 的验收顺序，以及界面中实例号与保留日志的现有显示，尚未收到本轮真实项目运行结果。
- 仅更新任务状态；文档本地链接、代码围栏及 git diff --check 检查通过，未改源码或重跑 Rust 测试。

## 下一步

1. 进入固定配置的 TUI，以仅 web 运行为起点，记下其日志标题实例号 #N；按 a 启动全部，检查三个项目状态及真实编译/开发日志。确认 web 仍为原 #N，再按 a 核对三项实例均不变化。若某项失败，保留项目 ID 与完整日志，并确认其他项保持运行。
2. 选中 web 按 x，确认 web 显示 STOPPED / Retained logs，而 weapp、middle 继续运行；再按 s，确认 web 使用新的实例号，较早实例日志标记 old，实际页面恢复。
3. 收到本轮结果后，按 [requirements.md](requirements.md) §3 继续离开/关闭终端与重连、全部停止、结束工作区，核对后代清理及端口释放；补齐失败反馈与 V → j/k → y 的目标终端粘贴体验。
4. fca30e4 顶栏版本显示已完成本地验证并提供源码包 /tmp/devctl-fca30e4.tar.gz，目标显示效果尚未收到明确反馈；可在本轮进入 TUI 时一并核对，之前的 e314939 包不包含该显示。

仅提交本步，不自动推送。完整真实项目验收仍未完成；MVP 完整验收前不推进 X03，计划见 [requirements.md](requirements.md) §4.1。
