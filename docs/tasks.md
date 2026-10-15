# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前状态：MVP 1.0.0 已发布

- 本轮起点 24fd7e9，工作区干净。2026-10-15 用户明确确认全部验收通过，授权发布 MVP，指定版本 1.0.0；关联 M1–M6、首版全部需求与 X01。
- 目标 WSL2 为 Ubuntu 22.04.5 LTS、x86_64，Node v24.18.0、npm 11.16.0。三个真实项目的脚本、单项/全部启停、重复启动保护、失败隔离、离开/关闭客户端后重连、停止与结束工作区流程均已由用户确认通过。
- 帮助框尺寸、版本显示、默认 devctl 进入 TUI、小写 v 从中间日志行选择与 Windows 粘贴一并通过本次用户确认，M6 已完成。
- Cargo 包版本与锁文件升至 1.0.0，CLI 和 TUI 自动复用包版本；同步需求、交互、路线及安装文档，新增 [版本说明](../CHANGELOG.md)。本次不改变功能代码和配置格式。
- 用户另行明确授权将四个附件上传至 Jackie733/devctl 并正式发布。版本提交 b7da949 与 v1.0.0 标签已推送，[v1.0.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.0.0) 已发布为 Latest 稳定版；仓库保持私有。

## 验证证据

- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings：通过。
- cargo test --locked --offline：在允许 Unix socket 的沙箱外完整回归 186 项通过（109 单元、77 集成），含 8 项真实 PTY 测试。
- RUSTFLAGS='-C linker=rust-lld' cargo build --release --locked --offline --target x86_64-unknown-linux-musl：通过，使用 rustc 1.98.1。file/readelf 确认 x86_64 static PIE、无 INTERP 和 NEEDED 项，避免本机 GNU 构建的 GLIBC_2.39 依赖。
- 直接对待发布静态程序运行 tests/support/tui_pty.py 的全部 8 个场景：通过，覆盖控制、重连、退出、默认配置发现、高输出、导航布局及两个复制通道。
- 临时 HOME 中使用附件更新器安装同一静态程序、重复安装、错误 SHA-256 拒绝且旧程序不变，以及安装后 --version：通过。bash -n scripts/devctl-update 通过。
- Markdown 本地链接、引用锚点、代码围栏与 git diff --check：通过。源码归档全部 55 个文件逐一核对，与 v1.0.0 标签提交一致。
- 四个附件上传后重新下载，逐字节核对与本地产物一致，SHA256SUMS 的三个文件校验全部通过；下载程序输出 devctl 1.0.0。GitHub Latest API 确认 tag_name=v1.0.0、draft=false、prerelease=false，全部附件 state=uploaded，target_commitish=b7da949022880399ba0f8fe797aa886e2e8795d7。
- 用户验收针对此前 WSL 安装的功能；本轮静态发布附件在当前 Linux 上验证，助手未连接目标 WSL 安装此附件，也未修改其现有后台。

## 发布交接与下一步

1. 本次发布完成，MVP 收尾结束。发布附件为 devctl-v1.0.0-x86_64-unknown-linux-musl、devctl-update、devctl-v1.0.0-source.tar.gz 和 SHA256SUMS；安装见 [wsl.md](wsl.md) §0。本地副本位于 /tmp/devctl-v1.0.0-release.dkfTLD。
2. X03 的公开下载、自动构建发布与 devctl update，以及日志增强尚未实现；范围见 [requirements.md](requirements.md) §4，架构见 [DESIGN.md](../DESIGN.md#53-mvp-后日志增强方案尚未实现)，具体先后等待用户安排。

升级需由用户在方便重启时显式结束旧工作区后台；新文件不会替换已运行的后台逻辑。助手无法反向 SSH 连接目标 WSL，本轮不自动操作目标项目或重启服务。
