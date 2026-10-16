# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前状态：1.2.0 已发布，待用户 WSL 验收（X03）

- 2026-10-16 用户明确授权完成 1.2.0 发布全流程，包括版本准备、GitHub Actions 工作流、提交、推送、标签及 Release；目标 WSL 下载、安装与验证留待用户完成。本轮未修改用户安装、停止真实项目、修改秘密或仓库可见性。
- 已发布 [v1.2.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.2.0)，为公开稳定版和 Latest。四个附件：devctl-v1.2.0-x86_64-unknown-linux-musl、devctl-update、devctl-v1.2.0-source.tar.gz、SHA256SUMS。
- 带说明的 v1.2.0 标签保持指向构建提交 09e735d98d3ef76a6c5414af7bdf0108dd7d8e0f，未移动标签；程序、源码包与发布说明的构建身份已核对。主分支后续 e355be4/7a3919f 完善发布补发与匿名验证配置，不改已发布程序和附件。
- 首次标签推送未产生运行记录，手动同标签只构建 [预检 38861461043](https://github.com/Jackie733/devctl/actions/runs/38861461043) 成功；随后通过当前主分支工作流、tag=v1.2.0 与 publish=true 显式补发，实际 checkout 仍为上述标签提交。
- [完整发布 38861746895](https://github.com/Jackie733/devctl/actions/runs/38861746895) 的 build/publish 均成功，先备齐草稿附件再公开；[匿名验证 38862477073](https://github.com/Jackie733/devctl/actions/runs/38862477073) 成功。当前自动流程包含构建、发布与发布后匿名验证，后者也能对既有版本独立执行；默认手动运行只构建，显式发布必须指定版本标签。

## 验证证据

- 本地 cargo test --locked --offline：236 项通过（143 单元、93 集成，含 10 个真实 PTY）；python3 tests/support/install_scripts.py：11 项通过。cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings、bash -n、actionlint 1.7.12 静态检查及 Markdown 链接/锚点/围栏检查通过；本地 actionlint 未启用未安装的 ShellCheck。
- 本地 1.2.0 musl 构建成功，版本、静态链接、隔离首次/重复安装、坏摘要保护、回退和配置保留通过，实际静态程序另跑 10 个 PTY 全部通过。干净工作区打包、错误标签保护、既有目录保护、源码归档及说明中的提交身份通过。
- GitHub 托管环境实际执行完整 236 项 Rust 测试、11 项安装器测试、静态产物安装/回退及 10 个 PTY，全部通过；日志 /tmp/devctl-v1.2.0-actions.log。
- 真实 Release 的四个附件已匿名下载并核对三条 SHA-256，实际程序版本/update 帮助/静态链接、同标签源码归档、隔离安装/回退通过；日志 /tmp/devctl-v1.2.0-release-verification.log。
- 本地共享出口仍触发匿名 API 限流，明确提示且不改变安装；GitHub 托管环境的无认证客户端已通过 latest 与指定版本查询、指定版本实际下载/校验/安装、离线回退、外部恢复和配置保留。验证日志 /tmp/devctl-v1.2.0-public-verify-actions.log。上述日志为临时文件，不保证长期保留；云端运行链接为持久交接入口。
- 最终差异、文档检查通过（12 个 Markdown 文档、89 个本地链接/锚点），远程引用核对确认标签仍解引用至 09e735d；仅提交本次交接，不再修改已发布标签或附件。

## 用户接手与后续

1. 按 [WSL 指南](wsl.md#0-安装-120-发布程序无需-rust) 下载/校验/安装，再按 [1.2.0 验收](wsl.md#42-120-安装与自更新验收) 核对真实环境。旧 1.0/1.1 没有 update，首次升级用外部更新器；回退至旧版后也用已下载的外部更新器恢复。
2. 发布不代替目标 WSL 和三个真实项目的体验验收，X03 尚未整体验收。跨已发布版本的默认升级可随下一次正常发版继续验证；同版本重装不冒充跨版本升级。
3. 日常发版和显式补发见 [发布维护](releasing.md)。后续推送、标签与发布授权按对应任务确认，候选功能不自动实施。
