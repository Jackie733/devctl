# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：修复 WSL 环境的日志保留与安装测试失败

- 本轮从 5224e6f 的干净工作区开始。用户反馈日志保留测试出现 storage=degraded，以及 Ubuntu/WSL sudo 启动提示导致 PATH 测试比较失败；原失败断言未打印完整保存诊断，不能仅据该截图确定用户机器的全部 I/O 原因。
- 已用测试进程专用的 LD_PRELOAD 包装 fsync 注入 500ms 延迟，在临时工作区复现旧日志测试失败，实际提示 757 条未保存、storage queue overloaded。原测试每隔 1ms 输出一条，把固定速率当作磁盘吞吐保证；这与 L03 有界队列允许过载退化的边界冲突。
- tests/logging_cli.rs 改为每批 100 条，等待存储线程处理后的查询响应，再用临时标记文件允许下一批，累计输出仍超过 4096 条缓存。无 degraded、跨内存回看、离线搜索与导出等断言继续保留；保存降级失败时打印完整 stderr，未放宽或关闭真实过载告警。
- 安装测试显式注入启动提示复现旧失败；改在交互 shell 内校验实际命令路径及运行版本，而不把整个 stdout 当作一个路径。另验证选到旧 Cargo 路径时确实失败；Bash/Zsh 的正常启动输出不影响判断。
- 只修改测试与说明，产品代码、保存策略、队列预算和安装脚本保持原样，源码版本仍为 1.1.0。排查说明见 [wsl.md](wsl.md#源码测试出现-degraded-或-shell-启动提示)。

## 验证证据

- 旧历史保留测试在 500ms 同步延迟下失败，新测试在同一注入条件下通过（1 项专项检查），全过程仅使用临时工作区；未修改用户三个项目或实际后台。
- `cargo test --locked --offline`：214 项全部通过（131 单元、83 集成，含 10 项真实 PTY）；本轮重新运行。
- `python3 tests/support/install_scripts.py`：10 项通过，包含启动提示和错误 PATH 的回归检查。
- `cargo fmt --check`、`cargo clippy --locked --offline --all-targets -- -D warnings`、Markdown 本地链接/锚点/围栏及 git diff --check 通过，已审查本轮差异。
- 未在用户目标 WSL 直接执行修复版；用户需要同步本轮源码后复验。实际运行中的 degraded 仍应根据完整诊断检查，不能因测试适配就当作正常保存。

## 下一步与交接

1. 用户在目标 WSL 同步修复源码，运行 cargo +stable test --locked --test logging_cli 和 python3 tests/support/install_scripts.py；通过后继续 wsl.md §2 的构建安装与 §4.1 的实际项目试用。
2. 若仍失败，保留新版断言输出的完整 stderr、源码提交和 Rust 版本继续排查；不以本机通过代替目标环境验收。助手未推送、发布、替换实际安装或停止用户后台。
3. 1.1.0 版本提交为 5224e6f，日志功能提交为 bfdd69f；功能边界见 [requirements.md](requirements.md) §4.2、[DESIGN.md](../DESIGN.md#53-mvp-后日志增强方案)。无跨退出书签、跨项目聚合或复杂筛选；AI 和 X03 仍保留后续讨论。
