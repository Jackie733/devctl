# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：X03 命令内自更新

- 2026-10-16 用户要求开始自更新开发。本轮从 d290330 的干净工作区开始，直接实现代码，不修改实际安装、真实项目后台、秘密或发布配置，不推送或发布版本。
- 用户最新决定后续将现有 `Jackie733/devctl` 私有仓库转为公开，沿用其 Releases；覆盖此前独立公开产物仓库选择。本轮未更改仓库可见性，未创建产物仓库。默认更新源已固定为现有仓库，不接入私有认证。
- 已实现独立于配置发现的 update 入口：检查最新稳定版/指定版、安装、离线回退；有界 HTTPS 下载、发布完整性/摘要/候选版本校验，嵌入并复用原安装器的锁、替换与备份规则。新增 serde_json 与 semver，未引入 HTTP 框架。
- 操作统一见 [interaction.md](interaction.md) §4；实现、不变量和失败边界见 [DESIGN.md](../DESIGN.md) §8。包版本仍为 1.1.0，新功能标注未发布，现有 1.1.0 附件不包含 update 命令。

## 本次验证

- `cargo test --locked --offline` 最终 235 项通过（143 单元、92 集成，包含 9 个新增更新场景）；日志 `/tmp/devctl-self-update-tests.log`。首轮沙箱禁止 Unix socket 导致 11 个已有测试报权限错误，在允许本地 socket、信号和 PTY 的环境完整重跑通过。
- `cargo test --locked --offline --test update_cli` 9 项通过；`cargo test --locked --offline --bin devctl update::tests` 3 项通过；`python3 tests/support/install_scripts.py` 11 项通过。使用隔离 HOME、模拟 HTTPS 下载与独立 sha256sum 校验基准，验证坏摘要的候选不会先被执行，未访问用户安装或后台。
- `cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、`git diff --check` 与 Python 文档检查通过（8 个 Markdown 文件、73 个本地链接/锚点、代码围栏闭合）。
- `cargo build --release --locked --offline --target x86_64-unknown-linux-musl` 成功，file 确认为 x86_64 static-pie；实际 musl 程序的版本、update 帮助及临时 HOME 下离线回退通过。包版本仍为 1.1.0，不以本地构建冒充新发布。
- 本轮未发布或执行目标 WSL 的真实在线升级/回退，未接入自动发布流程；当前私有仓库不满足匿名下载条件。以下旧版本发布验证属于历史交接。

## 必要交接：v1.1.0 已发布

- 用户于 2026-10-16 明确授权升级、创建 v1.1.0 标签、构建发布版本并推送 GitHub。上轮发布从 82f2fd7 的干净工作区开始，Cargo.toml/Cargo.lock 的包版本已为 1.1.0，未另增版本号。
- 交付已确认的 L01–L04 日志增强与 U01 搜索/浮窗阅读优化；保留 P04/P05 的结束确认与离开后后台继续运行语义。跨退出书签、聚合、复杂筛选、AI 与命令内自更新不属本次发布范围，X03 整体仍未完成。
- 发布仓库仍为私有 Jackie733/devctl，不改变可见性；发布入口为 [v1.1.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.1.0)。标签使用带说明的 v1.1.0，源码包直接从标签生成，发布说明包含构建提交。
- 附件沿用 v1.0.0 格式：`devctl-v1.1.0-x86_64-unknown-linux-musl`、`devctl-update`、`devctl-v1.1.0-source.tar.gz`、`SHA256SUMS`；源码顶层目录为 devctl-1.1.0。静态程序面向 Linux x86_64 / WSL2，无宿主 glibc 动态依赖。
- 同步 README、CHANGELOG、WSL 安装入口和日志文档的版本状态与锚点；安装步骤统一下载 1.1.0。升级与真实项目试用规则由 [wsl.md](wsl.md) §0、§2、§4.1、§5 维护。

## 上轮发布验证证据

- `cargo build --release --locked --offline --target x86_64-unknown-linux-musl` 成功，rustc 1.98.1；`--version` 为 devctl 1.1.0。file/readelf 确认 static-pie，程序头无 INTERP，动态段无 NEEDED。
- `cargo test --locked --offline`：223 项通过（140 单元、83 集成，包含 10 个真实 PTY 场景）；最终日志 `/tmp/devctl-v1.1.0-release-tests.log`。
- 发布前首轮发现历史分页 PTY 固定连按结束时旧页尚未返回的时序问题。两处场景现等待缓存由 100 条扩至 200 条后再翻页，保持原有历史内容、锚点和搜索断言；产品代码未改。修复后完整重跑通过。
- 直接使用发布 musl 静态程序执行全部 10 个真实 PTY 场景通过；修复测试时序后额外复核 history/disk-history 通过，日志 `/tmp/devctl-v1.1.0-artifact-pty.log` 与 `/tmp/devctl-v1.1.0-artifact-history.log`。
- `python3 tests/support/install_scripts.py`：10 项通过；另使用真实静态附件在临时 HOME 验证首次安装、重复安装、版本及错误 SHA-256 不替换程序，未触碰用户安装与配置。
- `cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、Markdown 本地链接/锚点/围栏及 `git diff --check` 通过。无新增依赖，发布构建对应的产品源码保持不变。

## 升级与后续

1. 按 WSL 指南用有仓库权限的账号下载附件并校验。1.0.0 用户先用旧程序显式结束各工作区后台、退出旧 TUI，再安装新程序；旧版内存日志不能补存。已有 1.1.0 日志后台时，已发布的搜索与浮窗改进只需更新客户端并重新进入。
2. 助手不替用户安装程序或停止真实项目。发布附件在受控 Linux 环境验证；目标 WSL 的三个真实项目、终端主题配色和完整日志体验仍待用户确认，发布不代替产品验收。
3. 必要交接：bfdd69f 实现有界持久化与历史检索，2db3313 修复 WSL 同步与 shell 提示测试，f95997d 实现原位搜索高亮与上下文，82f2fd7 完善浮窗。
4. 开发计划按 [roadmap.md](roadmap.md) §4 收敛：用户后续公开现有仓库；还需接入自动构建发布、发布含 update 的新版本，再完成目标 WSL 真实更新/回退与失败保护验收。当前仓库仍私有，线上检查预期访问失败；X03 尚未整体完成。
5. 单项/批量重启、依赖编排、自动重启、恢复、扩展平台、日志聚合/组合筛选与 AI 等保持候选状态，未经确认不自动实施。
