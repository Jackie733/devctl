# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：修复 WSL 日志复制与中间行选择

- 本轮起点 7649189，工作区干净。用户反馈日志复制后不能粘贴，且选择只能从首条可见日志开始；关联 M5、M6、L01、U01、X01，优先修复后继续验收。
- 用户已在目标 WSL 确认 command -v clip.exe 输出 /mnt/c/WINDOWS/system32/clip.exe，printf 测试命令执行成功，并明确反馈 Windows 中可以正常粘贴 devctl-copy-test。
- 本轮已实现本机 WSL 使用 clip.exe、其他环境/SSH 使用 OSC 52；剪贴板异步任务有并发与时间上限，失败保留选区。普通日志导航增加阅读光标，V 从该行开始选择。规则见 [interaction.md](interaction.md) §2.3，资源与状态边界见 [DESIGN.md](../DESIGN.md) §5。
- 新的 TUI 粘贴与中间行选择效果尚待同步构建后由用户确认；目标命令测试成功不等同于新 TUI 整体已验收。

## 已完成的用户试用与必要交接

- 目标环境为 Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；此前确认 /home/jackie/.cargo/bin/devctl 安装版本 0.2.0，Node v24.18.0、npm 11.16.0。
- 三项目配置与命令检查、帮助框尺寸、web 单项试用已通过。用户还确认全部启动与重复启动保护、单独停止 web 不影响另两项、web 再启动实例更新、旧日志 old 标记和页面恢复均通过。
- web 为 npm run dev → vite；weapp 为 npm run dev:weapp → Taro watch；middle 为 npm run dev → egg-bin dev；三个服务 autostart 均为 true。目录与配置见 [wsl.md](wsl.md)。此前 pnpm 错误没有新的复现反馈。
- 尚未单独收到目标 TUI 顶栏版本显示、q 离开重连、关闭客户端终端重连的明确结果，不因本轮报告复制问题而将这些项目标为通过。全部停止、结束工作区及失败反馈仍待验收。
- 助手无法反向 SSH 连接目标 WSL，沿用用户在 WSL 通过 scp 拉取当前 Linux 源码包的方式；旧包 fca30e4 不包含本轮复制修复。

## 本轮验证

- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings：通过。
- cargo test --locked --offline tui::：38 项 TUI 单元测试通过，覆盖中间行起点、追加日志与缩放时锚点、淘汰/翻页、Unicode 编码、剪贴板非零退出与超时。
- cargo test --locked --offline --test tui_cli：8 项真实 PTY 测试通过；使用隔离假 clip.exe 检查中文与换行、失败保留选区、复制期间新选区不被清除、帮助导航及退出后复制进程清理，不访问真实宿主剪贴板。OSC 52 场景独立解码核对中间起点的精确正文。
- cargo test --locked --offline：在允许 Unix socket 的沙箱外最终完整回归 186 项通过（109 单元、77 集成）。首次完整运行有一项原 PTY 测试在发送 Esc 后立即缩放时失败，补充等待帮助实际关闭后再缩放，相关场景与完整回归均重跑通过。
- 修改文档的本地链接、代码围栏、Python 3 AST 语法及 git diff --check 检查通过；已审查差异。未构建 release 产物，未在用户 WSL 安装本轮构建，不把受控测试记为实际 Windows 剪贴板验收。

## 下一步

1. 同步本轮源码并在目标 WSL 重新安装；进入日志后先 j/k 定位中间行，再 V → j/k → y，核对反馈包含 via clip.exe，在 Windows 编辑器粘贴检查正文、中文和换行。
2. 确认本轮修复后，补齐 q 离开/关闭客户端终端后的原工作区和实例重连结果，再验收 X 全部停止、重新启动无端口残留及 Q 确认结束后台。
3. 补齐失败反馈与其他未确认项，汇总首版结果。完整规则与验收依据见 [requirements.md](requirements.md) §3。

仅提交本步，不自动推送。MVP 完整验收前不推进 X03，范围见 [requirements.md](requirements.md) §4.1。
