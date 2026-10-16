# 发布维护

对应 X03，发布流程由 [release.yml](../.github/workflows/release.yml) 执行；目标为 Linux / WSL2 x86_64。发布授权、实际结果和用户验收进度记录在 [tasks.md](tasks.md)。

## 准备与触发

1. 同步 Cargo.toml、Cargo.lock、CHANGELOG 和当前安装文档的版本；完成本地检查并提交。稳定版标签必须为 `vMAJOR.MINOR.PATCH`，且与 Cargo 包版本相等。
2. 确认目标提交在主分支，取得本次推送、标签和发布授权后，创建带说明的标签并推送主分支与该标签。不移动或覆盖已发布标签。
3. 推送 `v*` 标签触发发布。手动运行 Release 工作流只进行测试、构建和产物上传，不创建或公开 Release，可用于发布前验证。

## 工作流与产物

构建任务只持 contents: read，使用固定提交的官方 checkout/upload-artifact，在 Ubuntu 22.04、Rust 1.98.1 下执行 fmt、clippy、完整 Rust 测试和安装器测试，再编译 x86_64-unknown-linux-musl。静态程序须通过无动态加载器/依赖检查、隔离安装/重复安装/错误摘要/回退检查及全部 10 个真实 PTY 场景。

[打包脚本](../scripts/package-release.sh) 要求工作区干净、可执行程序版本与 Cargo 版本及发布标签一致，输出目录必须尚不存在。程序、脚本和源码归档都来自同一提交；源码由 git archive HEAD 生成，顶层为 `devctl-<版本>/`，不包含 .git 或 target。输出：

- `devctl-v<版本>-x86_64-unknown-linux-musl`
- `devctl-update`
- `devctl-v<版本>-source.tar.gz`
- `SHA256SUMS`（覆盖上述三个附件）

另外生成内部 release-notes.md，从 CHANGELOG 当前版本条目提取说明并追加完整构建提交。工作流中间产物保留 7 天，最终下载入口为 GitHub Release。

发布任务依赖构建任务成功，使用内置 GITHUB_TOKEN 的 contents: write 与 actions: read，不需要新增个人令牌。下载同一次运行的产物并复核校验值，先创建草稿并上传全部附件，再公开并标记为 Latest。测试、构建或上传失败不会公开本次新版本。

同一标签的运行串行执行，不取消正在发布的任务。重试只允许更新同标签的草稿附件；已公开的 Release 拒绝覆盖。若发布后发现问题，修复并发布新版本；不要用同版本号替换程序。

## 发布后核对与交接

- 检查 Actions 构建与发布任务均成功，标签、构建提交和 Release 版本一致，四个下载附件齐全。
- 匿名下载程序、外部更新器和校验值，核对摘要、版本、静态链接及 update 帮助；用临时 HOME 验证实际附件安装和回退。
- 使用发布程序匿名执行 update --check、指定版本检查与安装，确认元数据和附件查询正常。API 限流时记录明确错误，保留现有程序；不以认证查询成功替代匿名客户端验收。
- 将 Release 与工作流运行链接交给用户，按 [WSL 指南](wsl.md#42-120-安装与自更新验收) 下载、安装、验证。目标 WSL 的真实项目体验由用户确认，自动化及发布成功不代表该项已通过。

1.0/1.1 不含 update 子命令，因此首次安装 1.2.0 仍用外部更新器。回退至旧版本后，可继续用已下载的外部更新器恢复新版；无需通过旧程序调用不存在的 update 命令。两个已发布版本间的默认自动升级，可在下一次正常发布时继续验证，不为验收制造空版本。
