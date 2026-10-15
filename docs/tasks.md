# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：帮助弹窗收紧，开始按顺序完成 MVP 验收

- 本轮起点 80b15e5，工作区干净。用户要求先优化截图中帮助弹窗的尺寸与空间，再按顺序收尾 MVP；助手直接实现与验证，用户最后审核。
- 关联 M5、M6、U01、X01。帮助框改为按换行后的内容确定高度，限制宽度并增加内外留白；小窗口滚动与翻页使用实际正文高度。交互规则统一见 [interaction.md](interaction.md) §2.5。
- 用户本轮提供目标环境输出：Ubuntu 22.04.5 LTS（Jammy）、x86_64；Zsh 的 whence -a devctl 仅列出 /home/jackie/.cargo/bin/devctl；devctl --version 实际为 0.1.0。仓库版本是 0.2.0，目标安装尚未同步；此前底部恢复正常的反馈不能证明已安装 0.2.0。
- 目标环境识别已完成，下一步为同步本轮源码、在目标 WSL 重新安装，再开始三个真实项目的 M6 完整流程。截图中的停止状态不代表完整启停与清理流程已经验收。

## 本轮验证

- cargo test --locked --offline help_：相关帮助渲染、翻页、长 Unicode 诊断和真实 PTY 导航检查通过。
- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings：通过；首次格式检查和 Clippy 提示已修正后重跑。
- cargo test --locked --offline：沙箱内首次运行因 Unix socket 操作被拒绝而失败；获准在沙箱外重跑后全部 182 项通过（106 单元、76 集成），含 7 项真实 PTY 测试，以及后台生命周期、失败隔离、进程组清理与有界日志。
- 新增两项回归检查，覆盖大屏内容适配、小屏详情与固定提示可达、缩放后的滚动边界，以及按实际正文高度翻页。
- 文档本地链接、代码围栏及 git diff --check 检查通过。未构建 release 产物，未在目标 WSL 安装或执行真实项目，不把本地测试记为用户验收。

## 必要交接与下一步

1. 同步本轮源码到目标 WSL，按 [wsl.md](wsl.md) 在方便停止项目时结束旧工作区，再重新安装。用 /home/jackie/.cargo/bin/devctl --version 确认输出 0.2.0，Zsh 中 rehash 后核对实际启动路径；同一版本下的新构建还需核对源码提交和帮助框效果。
2. 检查三个实际脚本与依赖，web 应为 npm run dev。实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据；脚本链与 stdin 需求仍待核实。旧后台保留创建时配置与环境，q 或 stop --all 不会使配置改动生效。
3. 按 [requirements.md](requirements.md) §3 完成单项、全部启动、重复启动、单项停止再启动、离开/关闭终端与重连、全部停止、结束工作区，核对后代清理及端口释放。
4. 核对帮助框尺寸、日志停止状态与保留输出、old 标记，以及 V → j/k → y 在目标 Windows Terminal 的粘贴体验；确认失败原因清楚且不影响其他项目。
5. 修复实际验收发现的问题、执行相应检查并记录用户结论，完成后再推进 MVP 后 X03。

仅提交本步，不自动推送。公开发布附件与 devctl update 尚未实现；现有 PATH 设置和外部更新器用于当前试用，不代表 X03 已完成。
