# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：Release 清单更新（X03）

- 用户在目标环境执行 1.2.0 的 update 时遇到匿名 GitHub REST API 限流，已确认改用随构建产物一同发布的 update.json；本轮范围为客户端、打包、Actions、测试和文档调整，不自动推送、创建标签或发布。
- 最新版本读取 releases/latest/download/update.json，指定版本读取 releases/download/v<VERSION>/update.json。校验清单格式、稳定版本、唯一平台、精确文件名、大小和摘要；获取明确版本后固定下载该版本程序，不再调用 REST API，不使用凭据。
- 构建与验证完成后从实际程序生成清单，SHA256SUMS 覆盖清单；清单和程序等五个附件一同上传草稿，成功后才公开并设为 Latest。沿用安装器的锁、备份、失败保护、离线回退和配置保留规则。
- 清单协议见 [DESIGN.md §8.1](../DESIGN.md#81-更新清单与下载约定)，操作与兼容边界见 [interaction.md §4](interaction.md#4-命令内自更新120)，发布顺序见 [releasing.md](releasing.md)。

## 验证证据

- 更新模块 2 项单元测试与全部 10 项更新 CLI 测试通过：清单缺失/无效/过大、版本与平台不符、重复平台、文件名、摘要、下载失败、长度不符和候选版本错误时保留安装与备份；检查与回退不依赖配置；请求 URL 固定到官方仓库和明确版本。
- 新增 4 项打包测试通过：临时 Git 仓库生成真实程序的清单并由真实客户端读取、检查、安装与回退；核对清单与 SHA256SUMS、源码归档和构建提交；拒绝标签不符、脏源码及覆盖已有目录。
- cargo fmt --check、cargo clippy --locked --offline --all-targets -- -D warnings、bash -n、actionlint 1.7.12 检查通过；本地 actionlint 未启用未安装的 ShellCheck。
- cargo test --locked --offline：235 项通过（142 单元、93 集成，含 10 个真实 PTY）；日志 /tmp/devctl-manifest-cargo-test.log。python3 tests/support/install_scripts.py：11 项通过。
- cargo build --release --locked --offline --target x86_64-unknown-linux-musl 成功；用实际静态程序再次运行上述 4 项打包/清单消费测试通过，release_artifact.py 的静态链接、首次/重复安装、错误摘要、回退和配置保留检查通过。
- 12 个 Markdown 文档、92 个本地链接/锚点、围栏及改动 Python 脚本语法检查通过。未推送或触发 Actions；新格式的公开网络验证须待新版本实际发布。

## 上一版本与后续交接

- [v1.2.0](https://github.com/Jackie733/devctl/releases/tag/v1.2.0) 已公开发布，原四个附件与标签保持原样，构建提交 09e735d98d3ef76a6c5414af7bdf0108dd7d8e0f。[发布运行](https://github.com/Jackie733/devctl/actions/runs/38861746895) 和 [原协议匿名验证](https://github.com/Jackie733/devctl/actions/runs/38862477073) 已成功，不代表新清单协议已在线发布或目标 WSL 已验收。
- 这次修改记录于 CHANGELOG 的未发布部分，尚未确定下一版本或发布新产物；1.2.0 已安装程序继续使用原 API 逻辑，不能仅靠修改远端工作流消除客户端限流。
- 后续以新版本号发布含清单的程序。旧客户端如仍受 API 限流，先用外部更新器安装新版；新客户端安装无清单的历史版本也使用外部更新器，不修改旧 Release。新格式的公开下载、Latest 入口和 Actions 真实运行待新版本发布后验证。
- 用户随后按 [WSL 指南](wsl.md#42-120-安装与自更新验收) 验收真实环境；本轮不修改真实安装、不停止项目。X03 尚未整体验收。后续推送、标签和发布按相应任务授权执行。
