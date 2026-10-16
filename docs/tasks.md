# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：X03 公开下载验证与限流诊断

- 2026-10-16 用户已将现有 Jackie733/devctl 仓库改为 Public。本轮从 0b7cf30 的干净工作区开始，继续验证公开入口，未更改线上设置、安装到用户目录、停止真实后台、推送或发布版本。
- GitHub CLI 只读确认 visibility=PUBLIC；匿名访问 releases/latest 跳转至 v1.1.0 且页面 HTTP 200，附件清单与更新器命名一致。
- 已匿名下载实际 1.1.0 musl 程序、devctl-update 和 SHA256SUMS；独立校验摘要后，在临时 HOME 使用真实附件安装，再用开发版 update --rollback 恢复原程序的精确内容。配置哨兵保持不变，不接触真实项目。
- 在线 update --check 遇到共享出口的匿名 API 限流：HTTP 403、x-ratelimit-remaining=0，错误正文明确为 API rate limit exceeded。本轮补充限流诊断及失败保护测试，保留有界下载、不自动重试、不引入认证或切换来源。固定附件下载不依赖 REST API 额度。
- README、WSL 匿名安装入口、需求、路线与实现文档已同步公开状态；操作与边界由 [interaction.md](interaction.md) §4、[DESIGN.md](../DESIGN.md) §8 维护。命令内在线更新需要 curl 7.76.0+。

## 本次验证

- cargo test --locked --offline --test update_cli：10 项通过，新增主/次 API 限流响应的明确提示、单次请求、原程序/备份/配置保留与暂存清理验证。
- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings、git diff --check 与 Python 文档检查通过（8 个 Markdown 文件、74 个本地链接/锚点、代码围栏闭合）。本轮只改更新失败诊断，未重跑无关的完整 Rust 套件或发布构建，上一轮完整结果仅作交接。
- 真实公开附件的匿名 HTTPS 下载、SHA-256 核对、隔离安装与开发版离线回退通过，线上限流提示正确，日志 /tmp/devctl-public-source-validation.log。临时 HOME 已清理，未留下用户安装改动。
- 限流解除前未完成命令内元数据查询成功后的完整在线安装；未在用户目标 WSL 执行真实升级/回退，未接入自动发布或发布新版本。

## 必要交接

- 0b7cf30 已实现开发版 update 检查、稳定版/指定版安装与离线回退；完整回归 235 项（143 单元、92 集成）、安装器 11 项、fmt、clippy、musl 构建与隔离回退通过。完整证据见该提交的任务记录。
- 包版本仍为 1.1.0；含日志增强的 v1.1.0 已发布，现有发布附件不包含 update 子命令。历史公开状态以本轮确认覆盖旧交接，首次安装可用 [WSL 指南](wsl.md) §0 的公开附件入口。
- M1–M6 的真实项目流程已验收；1.1.0 新增日志体验的目标 WSL 试用仍待用户确认，测试与下载成功不能替代体验验收。

## 下一步

1. 匿名 API 额度恢复后，验证开发版在线元数据检查与命令内安装；共享出口的限流不能作为仓库仍私有的证据。
2. 按 [roadmap.md](roadmap.md) §4 继续准备自动构建发布与含 update 的新版本，再在目标 WSL 完成真实升级/回退验收；X03 尚未整体完成。
3. 保持 AGENTS.md 对部署配置、推送与真实项目操作的约束；候选功能未经确认不自动实施。
