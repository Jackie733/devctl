# 当前开发任务

这里只保留当前任务与必要交接；历史见 Git，协作规则见 [AGENTS.md](../AGENTS.md)。

## 当前任务：WSL 试用反馈，web 启动命令与配置生效

- 用户确认 web 应为 npm run dev；已同步修正 [requirements.md](requirements.md) §1、[DESIGN.md](../DESIGN.md) 配置示例及 [wsl.md](wsl.md)，对应 W01、P02、P05。pnpm 不再作为三个启动命令的统一前置要求。
- 用户最新实例 #6 日志仍有 pnpm install，因 ERR_PNPM_IGNORED_BUILDS（core-js、esbuild）退出。代码审查确认后台只在创建时加载配置；停止/重新启动单项、q 后重连不会加载改动。配置与后台生命周期约定仍见 DESIGN §6，本轮不增加热重载。
- WSL 指南补充：显式指定同一配置，shutdown 成功后 validate / plan web / tui；shutdown 会停止工作区全部服务。若新后台仍报错，在项目目录直接运行 npm run dev 并检查 scripts / packageManager，区分后台旧配置与项目脚本间接调用 pnpm。没有用户项目源码，暂不能确定其调用链。

## 本轮验证

- 本轮仅修改文档；Python 检查 20 个本地 Markdown 链接、WSL 指南 10 个 sh 命令块语法（sh -n）、示例 TOML 解析与三个 npm 命令一致性，均通过；`git diff --check` 通过。不重复执行 Rust 测试。
- pnpm 的交互式依赖构建审批依据其官方 approve-builds 文档；未修改真实项目配置、依赖或锁文件，未在用户 WSL 执行启动与审批。

## 必要交接与下一步

- M1–M5 的首版功能及受控验证已完成。默认配置发现提交 ff0de7c，最近一次完整回归 **159 项通过**，fmt、clippy、release 构建通过；命令和环境见该提交的任务记录。未自动推送。
- 用户目标为 Windows WSL2；三个目录是 ~/code/deepmdt-frontend-pc、~/code/deepmdt-miniprogram-pro、~/code/rtc-middleware/api-proxy，配置示例见 [wsl.md](wsl.md)。目标 WSL 的发行版、架构、脚本/依赖及 stdin 交互要求仍待核实；本宿主没有这三个目录。
- 下一步先在 WSL 结束旧后台后仅启动 web，若仍失败，获取直接 npm run dev 的最新错误与 scripts / packageManager 输出；然后继续 M6 的三个真实项目完整流程验收，不能用本机测试通过代替用户体验确认。保留已有 Linux/后台保证与限制：有界近期日志，清理未知不虚报退出，不自动恢复 SIGKILL/断电或 WSL 关闭后的进程，不提前实现其他扩展。
