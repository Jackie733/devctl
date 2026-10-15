# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：M6 离开重连与日志复制验收

- 本轮起点 e5a2263，工作区干净。用户明确确认上一轮五项验收全部通过，现继续后台保留、关闭客户端终端后重连与目标终端复制体验。
- 当前关联 M6、P05、R02、L01、U01、X01。助手维护实现与记录，用户在目标 WSL 操作并反馈；发现实际问题后先定位并做最小修复。
- 目标环境：Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；此前确认路径 /home/jackie/.cargo/bin/devctl 与安装输出 0.2.0。fca30e4 顶栏版本显示的目标效果尚未单独确认。

## 已完成的用户试用

- 三项目磁盘配置校验与命令核对通过；帮助框尺寸合适，web 单项试用正常。
- 用户确认五项通过：仅 web 启动；a 启动全部且 web 沿用原实例；再次 a 不重复创建三项实例；x 只停止 web，另两项继续运行且 web 显示 STOPPED / Retained logs；s 再启动 web 后实例更新、旧日志标记 old、页面恢复。
- 上述反馈覆盖当前三个真实项目的正常启动链与部分日志、启停体验，不等同于关闭客户端、全部清理或失败反馈已经验收。

## 必要交接

- Node v24.18.0、npm 11.16.0，node/npm/pnpm 均位于 fnm 的 v24.18.0 安装目录。
- web 为 npm run dev → vite；weapp 为 npm run dev:weapp → npm run build:weapp -- --watch → taro build --type weapp；middle 为 npm run dev → egg-bin dev。三个服务 autostart 均为 true，目录与配置固定见 [wsl.md](wsl.md)。
- 此前实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 没有新的复现反馈，不保留为当前已知启动阻塞，也不据此断言历史原因；若再发生，保留完整日志排查。
- 已提供 /tmp/devctl-fca30e4.tar.gz 源码包与 WSL scp 拉取方式；助手未建立 WSL 远端连接。旧后台不会随更新重载配置与环境，处理约定见 [wsl.md](wsl.md) §5。

## 验证证据

- 最近代码提交 fca30e4 已通过 cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings、35 项 TUI 单元测试及 14 项 CLI/真实 PTY 集成测试。
- 本轮仅记录用户验收反馈并推进任务状态；本地文档链接、代码围栏及 git diff --check 检查通过。未改源码或重跑 Rust 测试，尚未收到本轮重连与复制结果。

## 下一步

1. 三项目保持运行，记下顶栏工作区短 ID 及各项目日志标题实例号。按 q 离开并重进同一配置，确认工作区、实例不变，项目仍运行且近期日志可见。
2. TUI 运行中关闭其客户端终端窗口或标签页，从新开的 WSL 终端重进同一配置，重复核对；这一步不关闭 WSL 发行版或 Windows。
3. 在日志面板用 V 选择单行、j/k 扩展多行、y 发送复制；在 Windows 文本编辑器粘贴核对正文和换行，确认没有 stdout/stderr、实例号、序号或 old 前缀。
4. 收到本轮结果后验证 X 全部停止、重新启动无端口残留、Q 确认结束后台；补齐失败反馈验收，汇总首版结果。用户要求的顶栏版本号可在进入 TUI 时一并确认。

仅提交本步，不自动推送。完整真实项目验收仍未完成；MVP 验收完成前不推进 X03，范围见 [requirements.md](requirements.md) §4.1。
