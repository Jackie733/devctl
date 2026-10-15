# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：功能版本升至 0.2.0，继续 WSL 真实项目验收

- 本轮起点 8b690e1，工作区干净。用户确认 WSL 使用 /home/jackie/.cargo/bin/devctl，重新打包编译后两行底部显示正常，旧客户端排查已结束；该反馈仅确认此问题恢复，不代表 M6 全部验收完成。
- 用户要求提升当前功能版本，Cargo.toml 从 0.1.0 升为 0.2.0；Cargo 自动同步 Cargo.lock 中本包版本，依赖与功能代码不变。关联 M5、M6、U01、X01，便于区分已包含界面优化的功能版本。
- [wsl.md](wsl.md) 同步安装后的预期版本输出与历史 0.1.0 构建的辨识说明。本轮仅本地版本提交，不创建发布附件或自动推送。
- X03 公开发布与 devctl update 继续安排在 MVP 验收后，范围见 [requirements.md](requirements.md) §4.1，顺序见 [roadmap.md](roadmap.md) §4。

## 本轮验证

- cargo check --offline：通过，自动更新锁文件，仅本包版本变化。
- cargo test --locked --offline --test cli --test config_discovery_cli：15 项通过，含 --version / -V 与包元数据一致、无配置时版本查询成功。
- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings 均通过。
- cargo run --locked --offline -- --version：实际输出 devctl 0.2.0。
- Python 3 检查两份修改文档的本地链接、代码围栏及包/锁文件版本一致性；git diff --check 通过，已审查差异。仅改版本与说明，未重跑完整回归或构建 release 产物；目标 WSL 的 0.2.0 安装仍待用户验证。

## 必要交接与下一步

- 同步本轮版本提交后，按 wsl.md 在方便重启时结束工作区，再重新安装；用 /home/jackie/.cargo/bin/devctl --version 确认输出 0.2.0，Zsh 中 rehash 后核对实际启动路径。
- cd2b276 已将 Zsh 作为安装主示例，含实际 Zsh 默认/自定义 ZDOTDIR、新交互/登录终端的 10 项脚本检查通过；8831bd5 的外部更新器也已受控验证。当前仍需可信构建方提供兼容二进制和校验值，没有正式发布附件。
- 日志停止状态与保留输出、old 标记的真实项目体验，以及 V → j/k → y 在目标终端的粘贴体验仍待用户核对；继续三个真实项目的 WSL2 M6 完整流程。
- web 为 npm run dev；旧后台沿用创建时配置与环境。实例 #6 的 pnpm install / ERR_PNPM_IGNORED_BUILDS 尚无新运行证据，发行版、架构、实际脚本/依赖和 stdin 需求仍待核实。
- 助手直接实现与验证，用户最后审核；仅提交本步，不自动推送，MVP 验收完成前不推进 X03 实现。
