# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：1.2.0 发布（X03）

- 2026-10-16 用户明确授权完成 1.2.0 发布全流程，包括版本准备、GitHub Actions 发布工作流、提交、推送、标签及 GitHub Release；目标 WSL 下载、安装与验证留待用户完成。这次授权覆盖本次发布所需的部署配置与推送操作，不扩展到其他配置、秘密或真实项目后台。
- 从 443d601 的干净工作区开始，远程 main 为 0b7cf30；仓库 Public、Actions 已启用、当前账号有写入权限，v1.2.0 标签尚不存在。
- 包版本改为 1.2.0，同步安装入口与更新说明；添加标签触发/手动只构建的自动流程、同提交打包脚本和真实静态产物安装验证。修复更新测试的固定未来版本假设，使其不随包版本升级失效。
- 本轮不替用户安装程序、停止服务或执行真实 WSL 验收。发布机制由 [releasing.md](releasing.md) 维护，用户验收由 [wsl.md](wsl.md) §4.2 维护。

## 验证与发布状态

- cargo test --locked --offline：236 项通过（143 单元、93 集成，含 10 个真实 PTY 场景）；日志 /tmp/devctl-v1.2.0-tests.log。python3 tests/support/install_scripts.py：11 项通过。
- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings、bash -n、actionlint 1.7.12 静态检查通过；actionlint 本地未启用未安装的 ShellCheck。Markdown 本地链接/锚点/围栏检查通过（12 个文档、87 个链接），git diff --check 通过。
- cargo build --release --locked --offline --target x86_64-unknown-linux-musl 成功；实际程序版本/静态链接、隔离首次/重复安装、坏摘要保护、回退和配置保留通过，静态产物另跑 10 个 PTY 场景全部通过，日志 /tmp/devctl-v1.2.0-artifact-pty.log。
- 提交后验证干净工作区的同提交打包、版本标签保护与源码归档，再推送标签执行线上工作流。发布工作流与 Release 尚未推送/运行，不将计划标为成功。

## 必要交接与后续

- 1.2.0 首次提供 update；旧 1.0/1.1 需通过外部更新器引导安装。回退至旧版后，用已下载的外部更新器恢复，不调用旧程序不存在的 update 子命令。
- 443d601 已验证 1.1.0 附件匿名下载、校验、隔离安装/回退，并补充匿名 API 限流诊断；0b7cf30 完整回归 235 项通过，后续限流测试使当前测试数预期为 236，实际结果以本轮为准。
- 发布后核对匿名下载、校验值、标签/提交与命令内更新入口，交付 Release 和工作流链接；目标 WSL 及三个真实项目的体验由用户确认。X03 在用户验收前保持未整体验收状态。
