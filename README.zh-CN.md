# devctl

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/devctl-mark-on-dark.svg">
  <img src="assets/branding/devctl-mark.svg" alt="devctl：三条独立服务线与一条统一控制线" width="96" height="96">
</picture>

用 Rust 编写的本地开发服务终端管理工具。在一个界面中启动多个项目、查看日志和停止服务；离开终端界面后服务继续运行，需要时重新连接。

[![Release](https://img.shields.io/github/v/release/Jackie733/devctl)](https://github.com/Jackie733/devctl/releases/latest)
[![Release workflow](https://github.com/Jackie733/devctl/actions/workflows/release.yml/badge.svg)](https://github.com/Jackie733/devctl/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[English](README.md) | **简体中文**

## 功能

- **多项目集中管理**：在各自目录运行独立的开发命令，单独启停服务或启动默认组合。
- **独立后台**：关闭 TUI 或客户端终端后服务继续运行，再次连接同一个后台，避免重复创建进程。
- **键盘操作界面**：支持 Vim 风格导航、项目与日志面板切换、上下文帮助及窄窗口布局；状态图标、焦点与快捷键强调、等待动效和短暂完成反馈让操作更清晰，无需 Nerd Font。这些视觉优化从 1.4.0 起提供。
- **日常日志排错**：查看 stdout/stderr、暂停跟随、翻阅保留历史、搜索字面文本并复制完整日志行与上下文；支持鼠标拖选部分文本，按 `y` 或右键复制，滚轮上下查看历史。宽日志面板使用本地时间，以精简且固定的输出来源与启动编号区分日志，重启后不再把历史行改标为 `old`。
- **有界日志存储**：近期记录保存在内存，历史记录按容量与期限保留；查询旧会话不启动服务。
- **校验与回退**：通过公开 Release 清单更新，验证 SHA-256 和候选程序版本，并保留离线回退副本。1.3.0 起显示终端阶段与下载进度，重定向时使用纯文本输出。

1.5.0 支持在 TUI 按 **a** 添加项目：粘贴目录，选择 npm/pnpm 项目已有脚本，或填写平时使用的命令，确认后保存。首次使用也不必手写配置，保存后项目保持停止。**A** 改为启动默认组合；命令行 `add`、`import` 仍可使用。无需选择环境版本，详见[添加项目](docs/interaction.md#adding-projects)。

更新可执行文件不会替换已运行的后台。添加保存遇到 EOF 时，先检查配置和状态；旧后台可能不支持添加，也可能是保存后回复丢失。需要切换后台时，明确关闭工作区再重开；关闭会停止该工作区全部服务，详见[恢复步骤](docs/interaction.md#adding-projects)。

1.6.1 提供首轮 [TUI 优化](docs/tui-design-research.md)：去掉启停状态的白闪，统一深浅配色与弹窗，切换面板仍保留选中项目，窄窗口按完整操作项显示提示。默认使用深色；`DEVCTL_TUI_THEME=light devctl` 使用浅色，`terminal` 跟随终端。另有减少动画、普通字符图标和无彩色选项，详见[界面设置](docs/interaction.md#layout-help-and-confirmation)。

## 平台支持

**1.9.0** 提供 **Linux x86_64（含 Windows WSL2）**、**macOS 14 及以上版本的 Apple 芯片** 程序。Linux 版本使用 musl 静态构建，无需安装 Rust，也不依赖特定 glibc 版本。请按[安装指南](docs/install.md)下载对应平台的程序；不支持 Intel Mac 和 Windows 原生运行。

开发命令需要能够非交互运行。目前支持依赖校验与计划展示，尚未实现运行时依赖编排、就绪探针、自动重启或机器重启后的恢复。已交付和候选功能的区别见[路线图](docs/roadmap.md)。

核心聚焦服务启停、日志、后台运行和轻量配置。此前未发布版本中的环境检查、版本绑定与自动安装计划已移出核心；使用过专用环境字段的配置需要[手动调整](docs/interaction.md#earlier-unreleased-environment-configurations)。数据库、Redis 自动化仍待重新评估。详见[范围说明](docs/requirements.md#directory-onboarding-and-environment-automation-a01a03)。

Agent 辅助排错已完成[调研与架构规划](docs/agent-research.md)，尚未实现。首个交付目标是在 devctl 内完成一次排查：从错误提示或选中日志进入，预览将提供的信息，查看分析、补充问题、定位引用日志，并在自行检查后刷新证据。宽终端并排显示日志与分析，窄终端切换查看；Agent 默认只获取选中服务的限定信息，额外访问需要明确开放，首阶段不执行修复。日常启停和日志功能仍可独立离线使用。

## 安装

可从 [GitHub Releases](https://github.com/Jackie733/devctl/releases/latest) 下载程序、外部安装器与校验值，按[安装指南](docs/install.md#prebuilt-binary)校验后安装到 `~/.local/bin/devctl`。无需 GitHub 账号、Token 或本地编译。详细参考文档以英文维护。

已安装 **1.2.1 或更高版本**时，先检查更新：

```sh
devctl update --check
```

安装或回退前，先结束各个正在使用的工作区后台并退出 TUI，再执行 `devctl update`。替换程序文件不会更新已经运行的后台。1.0/1.1 用户首次升级，或 1.2.0 用户受 API 限流影响时，先使用[外部安装器](docs/install.md#prebuilt-binary)迁移。

## 快速开始

直接运行 `devctl`，按 `a` 输入项目路径，按 `Tab` 补全目录（多个匹配时继续按 `Tab` 切换），即可添加；保存后新项目立即出现在列表并保持停止，已有服务继续运行。添加完成后按 `s` 启动，按 `q` 离开并保留后台，按 `Q` 确认结束整个工作区。

也可手动配置，将 `devctl.toml` 放在项目目录旁：

```text
workspace/
├── devctl.toml
├── web/
└── api/
```

```toml
version = 1

[workspace]
name = "local-dev"

[projects.web]
path = "./web"

[projects.api]
path = "./api"

[services.web]
project = "web"
cmd = ["npm", "run", "dev"]

[services.api]
project = "api"
cmd = ["npm", "run", "dev"]
```

目录与命令替换为实际值。命令采用参数数组，不隐式经过 shell；项目的相对路径基于配置文件目录解析，与运行 devctl 时的目录无关。

进入 `workspace/` 后执行：

```sh
devctl validate
devctl
```

TUI 连接空闲工作区，不自动启动服务。按 `s` 启动选中服务，或按 `A` 启动默认组合。服务默认属于该组合，可用 `autostart = false` 排除。

未指定 `--config` 时，依次查找 `./devctl.toml`、`~/devctl.toml`、`~/.config/devctl/devctl.toml`，使用第一个存在的文件。也可用 `devctl --config /path/to/devctl.toml` 明确指定工作区。

## 常用命令

| 命令 | 用途 |
| --- | --- |
| `devctl` | 打开或重新连接 TUI，不自动启动服务 |
| `devctl start web` | 启动一个服务，必要时创建后台 |
| `devctl start --all` | 启动默认组合，即 `autostart = true` 的服务 |
| `devctl restart web` | 停止并确认进程组清理后重启一个服务 |
| `devctl restart --all` | 停止全部活动服务并重启默认组合 |
| `devctl reload` | 在不打断运行中服务的前提下重载已停止服务的配置变更 |
| `devctl status` | 查询已有后台的服务状态 |
| `devctl logs web --limit 100` | 查看一页有界日志 |
| `devctl log-sessions` | 列出保留的日志会话 |
| `devctl logs web --after 0 --contains 'Error'` | 在保留输出中搜索字面文本 |
| `devctl stop web` | 停止一个服务并清理对应进程组 |
| `devctl stop --all` | 停止全部服务，保留后台 |
| `devctl shutdown` | 停止全部服务并结束后台 |
| `devctl update --rollback` | 离线交换当前安装与上一版本 |

工作区命令都支持 `--config PATH`。`update`、`--help` 和 `--version` 不依赖工作区配置。前台运行可使用 `devctl run web` 或 `devctl run --all`，Ctrl-C 会清理本次选择的服务。

## TUI 快捷键

| 按键 | 操作 |
| --- | --- |
| `j/k`、`↑/↓`、项目面板左键/滚轮 | 根据当前焦点或点击区域选择项目或移动日志光标 |
| `h/l`、`Tab`、`1/0` | 切换项目与日志面板 |
| `Enter`、`Esc` | 进入日志或返回项目；具体行为取决于当前模式 |
| `s/x/r` | 启动、停止或重启选中项目（断开时 `r` 触发重连） |
| `a`、`e/d` | 添加项目，或编辑/删除已停止的选中项目 |
| `u` | 热重载 `devctl.toml` 中已停止服务的变更，保持运行中服务不中断 |
| `A`、`R`、`X` | 启动默认组合、重启默认组合或停止全部服务 |
| `c` | 重新连接并同步当前后台工作区 |
| `gg/G`、`PageUp/PageDown` | 翻阅日志；非选择模式下 `G` 恢复跟随 |
| `p` | 暂停或恢复跟随 |
| `/`、`n/N` | 搜索字面文本，定位下一条或上一条匹配 |
| `v`、`j/k`、`y` | 选择完整日志行并请求复制 |
| 鼠标左键拖选、`y` 或右键 | 选择并复制部分日志文本；`Esc` 取消 |
| 鼠标滚轮 | 在项目面板切换项目，或在日志内容区上下翻阅并加载更多保留历史 |
| `?` | 打开上下文帮助 |
| `q`、`Ctrl-C` | 离开客户端，后台服务继续运行 |
| `Q` | 确认停止全部服务并结束工作区 |

本机 WSL 通过 `clip.exe` 复制，本机 Mac 使用 `pbcopy`；其他环境与 SSH 使用 OSC 52，需要显示终端支持。选择模式、搜索边界、分页与错误反馈见[使用参考](docs/interaction.md)。

## 文档

- [Logo / 图标与使用规范](assets/branding/README.md)：完整图形用于 README、文档及项目标识，提供透明矢量图和图片。TUI 和帮助页保留清楚的名称、版本、状态及操作信息，不放装饰性 Logo 或简化替代符号，顶部维持原来的紧凑布局。
- [安装、更新、回退与源码构建](docs/install.md)
- [配置、CLI、TUI 与日志参考](docs/interaction.md)
- [WSL 设置与实际验收](docs/wsl.md)
- [架构与资源限制](DESIGN.md)
- [需求与验收标准](docs/requirements.md)
- [路线图](docs/roadmap.md)与[版本记录](CHANGELOG.md)
- [发布维护](docs/releasing.md)

## 参与贡献

欢迎问题反馈、文档改进与范围明确的 PR。开发环境、检查命令和问题反馈要求见 [CONTRIBUTING.md](CONTRIBUTING.md)。较大的产品行为调整请先讨论。

程序本身由 Rust 实现，日常使用无需 Python。仓库中的 Python 用于自动测试和发布工具，仅使用标准库；终端测试已将共用工具与具体场景分开，目录分工和运行方式见[开发说明](CONTRIBUTING.md#python-support-code)。

维护者可在干净的 `main` 检出中，提交审核过的 `Unreleased` 更新记录后执行 `./scripts/release 1.7.0`（替换为目标版本）。脚本自动更新版本、测试、提交、打标签、推送，并等待双平台发布与公开验证；`--resume` 可继续查看已有标签的发布结果。详见[发布维护](docs/releasing.md)。

## 许可证

本项目采用 [MIT 许可证](LICENSE)。
