# WSL2 安装与项目配置

目标环境是 Windows WSL2、Ubuntu 22.04.5 LTS、x86_64；2026-10-15 用户确认 MVP 完整验收通过，发布版本为 1.0.0。当前源码版本为 **1.1.0**（日志增强），尚无该版本的远程发布附件；自行构建按 §1–§2，真实项目试用按 §4.1。以下保留安装、配置与验收流程，验证证据见 [tasks.md](tasks.md)。

## 0. 安装 1.0.0 发布程序（无需 Rust）

[v1.0.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.0.0) 提供 Linux x86_64 musl 静态程序，不依赖宿主 glibc 版本；目标架构应为 `uname -m` 输出的 `x86_64`。仓库保持私有，需要有访问权限的 GitHub 账号；不能直接把私有附件 URL 交给不支持认证的外部更新器。可在已安装并登录 GitHub CLI 的 WSL 中下载：

```sh
gh auth status
gh release download v1.0.0 --repo Jackie733/devctl \
  --pattern devctl-v1.0.0-x86_64-unknown-linux-musl \
  --pattern devctl-update --pattern SHA256SUMS \
  --dir "$HOME/Downloads/devctl-v1.0.0"
cd "$HOME/Downloads/devctl-v1.0.0"
sha256sum --check --ignore-missing SHA256SUMS
```

也可从 Release 页面登录下载相同三个附件，再进入下载目录校验。校验全部成功后，已有版本的用户先按 §5 结束各工作区旧后台并退出 TUI，再安装；首次安装直接执行：

```sh
binary_sha=$(awk '$2 == "devctl-v1.0.0-x86_64-unknown-linux-musl" { print $1 }' SHA256SUMS)
bash ./devctl-update ./devctl-v1.0.0-x86_64-unknown-linux-musl "$binary_sha"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
rehash
command -v devctl
devctl --version
```

预期输出 `devctl 1.0.0`，安装位置为 `~/.local/bin/devctl`。长期 PATH 设置见 §2，项目配置见 §3；已有配置继续使用。下方 §1–§2 是从源码编译的替代方式。

## 1. 在 WSL2 准备源码与工具

在 Windows PowerShell 中可用以下命令核对发行版和 WSL 版本：

```powershell
wsl -l -v
```

其余命令均在目标 WSL2 的 Linux 终端执行。编译时沿用目标环境的 CPU 架构与系统库，运行的是 Linux 二进制。当前实现依赖 Unix socket、进程组、信号及 /proc，Windows 原生 exe 不属于当前目标。

源码建议放在 WSL 的 `~/code/devctl`，三个项目也使用已确认的 WSL 路径。微软建议使用 Linux 工具开发时将项目放在 WSL 文件系统中；参考 [WSL 文件系统说明](https://learn.microsoft.com/en-us/windows/wsl/filesystems) 与 [WSL 基本命令](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)。

构建 1.1.0 需要包含本轮升级提交的源码工作副本，不能使用 `v1.0.0` 标签或旧 Release 源码包。先同步源码到 WSL，再核对 `Cargo.toml` 中包版本为 `1.1.0`；若本地提交尚未推送，另一个克隆仅执行 git pull 不会取得它，应先同步该提交或源码文件。无需复制 target 编译缓存。`devctl --help` 会列出默认配置查找路径。

若使用源码归档，先解压到独立目录，避免覆盖已有工作副本；归档不含 .git 或 target，跳过下面的 git log。

```sh
cd "$HOME/code/devctl"
git log -3 --oneline
rustc --version
cargo --version
node --version
npm --version
command -v node npm
```

Node.js 和 npm 使用目标 WSL 中安装的 Linux 工具，Node 版本沿用各项目要求；启动命令直接继承运行 devctl 时的 PATH，不通过交互式 shell 加载别名。确认三个开发命令能在各自项目目录手动运行；若项目脚本内部使用 pnpm，再按项目约定准备 pnpm。

尚未安装 Rust 时，使用 [Rust 官方安装说明](https://rust-lang.org/tools/install/) 安装 rustup 和当前 stable 工具链；已有 rustup 可执行 `rustup update stable`。仓库目前以 Rust 1.98.1 实测，不把运行锁所需的 1.89 当作整个依赖树的最低版本承诺。Ubuntu/Debian 缺少构建工具时可先执行：

```sh
sudo apt update
sudo apt install -y build-essential curl git python3
```

## 2. 构建、测试并安装 1.1.0 源码版

先在目标 WSL 的源码目录完成构建和受控测试，不必提前停下日常项目。以下假定源码位于 `~/code/devctl`，使用 rustup 管理工具链，且没有自定义 Cargo target/target-dir：

```sh
cd "$HOME/code/devctl"
head -n 5 Cargo.toml
rustup update stable
rustup component add --toolchain stable rustfmt clippy
cargo +stable fmt --check
cargo +stable clippy --locked --all-targets -- -D warnings
cargo +stable test --locked
python3 tests/support/install_scripts.py
cargo +stable build --release --locked
./target/release/devctl --version
```

先核对包版本为 1.1.0。自动测试预期 220 项 Rust/集成检查（其中 10 个真实 PTY 场景）全部通过；安装脚本另有独立 Python 测试，使用临时 HOME，不替换当前安装。测试使用临时配置、进程和状态目录，不启动上述三个真实项目、不写入真实 Windows 剪贴板。最后一行必须输出 `devctl 1.1.0`。

`--release` 生成优化构建，`--locked` 使用已有锁文件；首次在 WSL 构建不加 `--offline`，需要下载尚未缓存的依赖。输出默认位于 `target/release/devctl`；如有 Cargo target/target-dir 配置，以实际输出目录为准。参考 [Cargo build 文档](https://doc.rust-lang.org/cargo/commands/cargo-build.html)。在目标 WSL 内构建无需交叉编译 musl，也无需用 sudo 执行 Cargo。

确认构建和测试通过、方便停止项目后，退出旧 TUI，并使用当前已安装的旧版结束工作区：

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" shutdown
```

此命令会停止该工作区全部项目；多个配置需要分别处理。已有后台时必须成功结束，清理失败先排查。若本就没有后台，可直接继续；1.0.0 的旧内存日志不能通过升级补存到磁盘。仅按 q 离开或 stop --all 均不等于结束后台。

在同一源码目录安装到惯用的 `~/.local/bin/devctl`，沿用已有更新器保留上一安装备份：

```sh
devctl_build_sha=$(sha256sum target/release/devctl | awk '{print $1}')
bash scripts/devctl-update ./target/release/devctl "$devctl_build_sha"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
rehash
command -v devctl
devctl --version
```

这里自动取得本机构建文件的 SHA-256，无需手抄占位符。预期实际执行路径为 `~/.local/bin/devctl`、版本为 `devctl 1.1.0`。安装只替换用户目录的程序，不修改已有工作区配置、不启动项目；首次安装无上一版备份，后续替换会保留 `devctl.previous`。目标 shell 为 Zsh，Bash 用户将 rehash 改为 hash -r。如路径仍不同，用 `whence -va devctl` 排查别名或旧 PATH。

仍可用 `cargo +stable install --path . --locked --force` 安装到 Cargo 根目录（通常 `~/.cargo/bin`），但它不会更新优先级更高的 `~/.local/bin/devctl`；本指南统一使用上面的构建加更新器方式。PATH 持久化见下节，回退见 §5。

### 一次性持久化 PATH

目标交互 shell 已确认是 Zsh。在目标 WSL 的源码目录执行 [PATH 设置脚本](../scripts/setup-path.sh)，无需 sudo：

```sh
ZDOTDIR="${ZDOTDIR:-$HOME}" bash scripts/setup-path.sh zsh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
rehash
command -v devctl
devctl --version
```

第一行中的 `bash` 是脚本解释器，最后的 `zsh` 指定写入 Zsh 启动文件；不要改成 `zsh scripts/setup-path.sh`。显式传递 ZDOTDIR 可以处理它只在当前 Zsh 中设置、尚未导出的情况。脚本向 `${ZDOTDIR:-$HOME}/.zshrc` 追加 PATH，保留已有内容，相同配置不重复追加；未设置 ZDOTDIR 时对应 `~/.zshrc`。

第二行使当前终端立即生效，rehash 刷新命令缓存；后续新开的交互 Zsh 会读取 .zshrc，无需每次执行 export。参考 [Zsh 启动文件说明](https://zsh.sourceforge.io/Doc/Release/Files.html)。设置后重新打开一个 WSL 终端，用 `command -v devctl` 再核对一次。若此前误执行了 `bash scripts/setup-path.sh bash`，再执行上述 Zsh 命令即可，不需要删除已有 .bashrc。

`~/.local/bin` 优先于 `~/.cargo/bin`，方便下节使用独立更新器；自定义 Cargo 安装目录需自行加入 PATH。Bash 用户可改用 `bash scripts/setup-path.sh bash` 和 `hash -r`，其启动文件规则见 [Bash 说明](https://www.gnu.org/software/bash/manual/html_node/Bash-Startup-Files.html)。非交互任务应显式设置 PATH 或使用绝对路径。

这里持久化的是可执行文件搜索路径，不是项目业务变量或 Node 版本管理器的初始化；后两者仍按项目约定配置，并在新环境中重建 devctl 后台。

## 3. 写入三个真实项目的配置

配置位置约定为 `~/.config/devctl/devctl.toml`。下面整段命令中的 `$HOME` 由 WSL shell 展开，最终写入 TOML 的是实际 Linux 绝对路径；devctl 不自行展开 TOML 中的 `~` 或 `$HOME`。若这个位置已有配置，保留原文件并合并对应字段。

创建目录后，将以下 TOML 内容写入该配置文件；路径替换为 WSL 中实际用户的 home 路径。为避免手工替换，首次创建也可执行下面的完整命令：

```sh
mkdir -p "$HOME/.config/devctl"
if [ ! -e "$HOME/.config/devctl/devctl.toml" ]; then
    cat > "$HOME/.config/devctl/devctl.toml" <<EOF
version = 1

[workspace]
name = "deepmdt"

[projects.web]
path = "$HOME/code/deepmdt-frontend-pc"

[projects.weapp]
path = "$HOME/code/deepmdt-miniprogram-pro"

[projects.middle]
path = "$HOME/code/rtc-middleware/api-proxy"

[services.web]
project = "web"
cmd = ["npm", "run", "dev"]
autostart = true

[services.weapp]
project = "weapp"
cmd = ["npm", "run", "dev:weapp"]
autostart = true

[services.middle]
project = "middle"
cmd = ["npm", "run", "dev"]
autostart = true
EOF
else
    printf '%s\n' '配置已存在，请保留原文件并合并项目配置。'
fi
```

三个路径由用户确认；命令沿用此前确认的开发方式。当前会话宿主没有这三个目录，尚未检查其 package.json、依赖安装或运行结果。`autostart = true` 让 `a` / `start --all` 选择这些项目，进入 TUI 本身仍不启动服务。配置格式与路径解析的主要说明见 [DESIGN.md](../DESIGN.md) §2。

### 只读检查启动脚本与工具链

在目标 WSL 的普通 Zsh 终端执行以下命令，记录输出；可从任意目录运行。这些命令只读取版本、项目元数据和 devctl 配置，不安装依赖或启动项目。

```sh
node --version
npm --version
command -v node npm pnpm
npm --prefix "$HOME/code/deepmdt-frontend-pc" pkg get scripts engines packageManager
npm --prefix "$HOME/code/deepmdt-miniprogram-pro" pkg get scripts engines packageManager
npm --prefix "$HOME/code/rtc-middleware/api-proxy" pkg get scripts engines packageManager
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" plan
```

核对 web、中台的 dev 与小程序的 dev:weapp 脚本，并检查相关 pre/post 脚本、是否间接安装依赖、调用 pnpm 或要求交互输入；Node 版本与包管理器遵循各项目约定。未声明 engines 或 packageManager 不代表检查失败，缺少 pnpm 也只有在实际脚本需要它时才需处理。这一步不证明依赖已安装或项目可正常运行，后续仍需逐项启动验证。此前 pnpm 错误的排查见 §5。

## 4. 校验与进入

```sh
devctl validate
devctl plan
devctl
```

程序现在会自动发现配置，默认进入 TUI，具体入口与配置优先级见 [交互方案](interaction.md) §1。若当前目录或 ~/devctl.toml 存在更高优先级的配置，短命令会选中它；需要固定使用本节的工作区时，可运行 `devctl --config "$HOME/.config/devctl/devctl.toml"`。找不到配置会明确报错，不会生成空配置。validate 校验字段、路径与引用，不执行 npm/pnpm，也不保证开发命令或项目依赖可用。

按 [交互方案](interaction.md) 验收：先只启动 web，再启动全部；检查日志、单项停止和重新启动；离开后重新连接；最后全部停止、结束工作区。客户端关闭后的保留行为也需在目标 WSL2 中实测。WSL 的 `wsl --shutdown` / `wsl --terminate` 会终止对应运行环境，后台运行不提供跨 WSL 关闭或 Windows 重启的恢复保证。

### 4.1 1.1.0 日志功能的真实项目试用

先核对版本并使用已有配置进入；不要重写 §3 的配置：

```sh
devctl --version
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" tui
```

1. 先选 web 按 s 启动，确认实时日志和 `save=saving`；再逐个启动 weapp、中台，确认输出和控制范围各自独立。
2. 日志焦点按 p 暂停，新增计数增长而正文位置不移动；上滚查看历史，切项目后再切回保留阅读位置；G 回尾部跟随。
3. 按 / 输入实际出现的字面文本、Enter 搜索，n/N 查下一个/上一个，命中后关键词高亮、前后非匹配日志仍显示，用 v/y 复制上下文并在 Windows 记事本核对；Esc 取消输入或清除搜索高亮，保留阅读位置。搜索大小写敏感，开始时固定输出上界，重新 / 才包含之后的新记录。
4. 在历史阅读位置按 q 离开，再进入同一个 TUI，默认跟随当前会话最新输出，不恢复退出前的旧位置。q 后服务应仍在运行。
5. 在方便停止全部项目时，按下面的 CLI 操作验证正常结束后历史仍可查；记录会话 ID，重建后台后再用 --session 查询旧会话，确认没有混入当前日志。

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" shutdown
devctl --config "$HOME/.config/devctl/devctl.toml" log-sessions
devctl --config "$HOME/.config/devctl/devctl.toml" logs web --limit 20
```

显式历史读取示例为 `devctl --config "$HOME/.config/devctl/devctl.toml" logs web --session <会话ID> --limit 20`；将占位符替换为列表中的实际 ID。无后台时日志查询不创建后台或启动项目。服务尚未产生输出时没有对应保存会话，不把它当作保存失败。

日志默认保存到 `~/.local/state/devctl/logs` 下；若设置绝对 XDG_STATE_HOME，则使用 `$XDG_STATE_HOME/devctl/logs`，CLI 的 stderr 和 TUI 帮助显示实际会话目录。`.dlog` 是结构化分段，通过 logs 读取，不直接用 tail。默认无需新增配置；参数、配额与故障边界以 [存储设计](../DESIGN.md#持久身份格式与预算) 为准，完整操作以 [日志交互](interaction.md#28-日志保存会话与文本查找源码增强尚未发布) 为准。

超过内存缓存的回看、容量/期限清理和故障退化先由受控自动测试覆盖；不建议为验收主动占满 WSL 磁盘或修改正在使用的历史文件。真实项目试用需关注输出频率、保存降级提示、查找体验和启停是否仍流畅，反馈实际命令及提示。

## 5. 更新与排查

更新 devctl 二进制或变更配置/Node 环境前，先结束使用该配置的后台；新客户端不会替换仍在运行的旧后台或重新加载其配置和环境。如果管理多个配置，需要逐一结束对应后台。shutdown 会停止该工作区的服务；1.0.0 的内存日志会丢失，1.1.0 已成功保存的历史仍按保留策略可查，安排在方便重启的时候进行；`q` 和 `stop --all` 都不会结束后台。

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" shutdown
```

后台本就不存在时，shutdown 报无法连接；以当前状态为准，不据此创建后台。清理失败或状态不确定时先排查，不忽略错误继续更新。

### 安装外部更新器

按 §2 设置 PATH 后，在目标 WSL 的源码目录安装一次 [外部更新器](../scripts/devctl-update)：

```sh
install -m 755 scripts/devctl-update "$HOME/.local/bin/devctl-update"
devctl-update --help
```

之后更新器可独立于源码运行。它支持本地 Linux 二进制或 HTTPS 下载地址，安装到 `~/.local/bin/devctl`；不会修改工作区配置，也不会自动停止后台或重启项目。它不支持压缩包、Windows exe 或需要登录认证的下载流程。

### 准备二进制与校验值

优先使用 §0 的发布附件与校验值。仓库尚无自动发布流程；自行构建时，应在与目标 WSL 架构、系统库兼容的 Linux 环境中构建：

```sh
cargo +stable build --release --locked
sha256sum target/release/devctl
```

将 `target/release/devctl` 传到目标 WSL，例如保存为 `~/Downloads/devctl`，同时从构建方取得输出中的 64 位 SHA-256。WSL 收到预编译程序后无需 Rust 或 Cargo。目标架构可用 `uname -m` 核对；仅架构相同仍不足以保证系统库兼容，需要实际运行验证。当前发布附件通过 §0 的认证下载取得；只有无需认证的 HTTPS 来源才能直接交给外部更新器。

SHA-256 用于核对文件是否符合预期；程序与预期校验值应来自可信的构建或发布来源。更新器会在执行候选程序前比较校验值。

### 更新与回退

确认上面的 shutdown 成功、旧 TUI 已退出后运行。将第二个参数替换为实际校验值，示例占位符不能直接使用：

```sh
devctl-update "$HOME/Downloads/devctl" '<64位SHA-256>'
hash -r
command -v devctl
devctl --version
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" tui
```

有正式下载地址后，第一行可改成 `devctl-update 'https://实际发布地址/devctl' '<64位SHA-256>'`。下载与重定向均限定 HTTPS。版本与架构由下载来源选择，更新器不自动猜测或追踪 latest；当前 `--version` 仍是 Cargo 包版本，具体构建可通过 SHA-256 区分。

更新器在安装目录暂存文件，核对 SHA-256 并限时执行 `--version`，成功后才通过同一文件系统的 rename 替换程序。校验或下载失败时已安装程序保持原样；重复安装相同内容不会覆盖备份。同时运行两个更新器时第二个会失败。`--version` 检查不代替完整项目试用。

每次替换前将原 `~/.local/bin/devctl` 保存为 `~/.local/bin/devctl.previous`。需要回退时，先结束当前版本启动的各后台，再执行：

```sh
devctl-update --rollback
hash -r
devctl --version
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" tui
```

回退会交换当前版与上一版，仅保留一个备份。首次从 Cargo 迁移时，尚无 `devctl.previous`；原安装保留在 `~/.cargo/bin/devctl`，可通过该绝对路径使用。若安装目标或备份是符号链接，脚本拒绝覆盖，需先检查已有安装方式。

继续使用源码安装也可以：在更新后的源码目录执行 `cargo +stable install --path . --locked`。若已安装 `~/.local/bin/devctl`，它会优先于 Cargo 版本；不要混用两套更新入口后仅凭版本字符串判断实际执行位置。

### 源码测试出现 degraded 或 shell 启动提示

旧源码的历史保留测试每隔 1ms 输出一条日志；WSL 文件同步或调度较慢时可能超过有界保存队列，出现真实的 `storage queue overloaded`。修复后的测试每批输出 100 条，等待存储线程处理后的查询响应再继续，仍要求全部保留场景无 degraded，并在失败时输出完整保存诊断。过载退化本身由独立测试验证。

Ubuntu/WSL 的交互 shell 可能输出 sudo 使用提示，这是正常启动输出。修复后的安装测试在 shell 内检查 devctl 的实际路径和运行版本，允许启动提示存在；测试还验证优先选到旧 Cargo 安装时确实失败。同步修复源码后，在源码目录执行：

```sh
cargo +stable test --locked --test logging_cli
python3 tests/support/install_scripts.py
```

若仍失败，保留新版断言输出的完整 stderr、源码提交和 Rust 版本继续排查；实际使用中的保存降级仍表示日志可能未完整保存，应按提示中的 I/O 错误或过载原因检查。

### 更新后底部仍显示三行

143dce2 已将主界面底部固定为一行状态和一行快捷键，054665a 又将日志状态与 FOLLOW/READ 分开显示。若仍看到底部独立的 `q leave (keep running) | ? help | a start default` 一行、日志标题为 `Logs: web | FOLLOW`，应先检查是否仍在运行旧 TUI 客户端。

按 q 离开界面，在 WSL 的源码目录检查：

```sh
git log -5 --oneline
whence -va devctl
```

Zsh 的 whence 会显示别名、函数及 PATH 中的各个安装位置。特别注意 ~/.local/bin/devctl 优先于 ~/.cargo/bin/devctl：cargo install 只更新 Cargo 安装位置，可能没有更新实际启动的文件。历史试用版本为 0.1.0、0.2.0，发布附件仍为 1.0.0，当前源码为 1.1.0；同一版本下的不同构建仍需结合源码或文件校验值核对。

确认源码包含上述提交后，可用显式源码运行隔离安装路径问题：`cargo +stable run --locked -- --config "$HOME/.config/devctl/devctl.toml" tui`。它会按需编译当前源码；若此时底部为两行，再更新实际安装位置。布局由客户端绘制，结束后台本身不会更新旧客户端；后台升级规则仍按本节前文执行。

### 修改 web 命令后仍出现 pnpm 日志

先确认 `[services.web]` 的 `cmd = ["npm", "run", "dev"]`。按 q 离开 TUI、停止后重新启动单项服务或执行 stop --all，都会保留原后台，不能使配置改动生效。上一段的 shutdown 会停止该工作区的全部服务并结束后台；确认它成功后，再执行：

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" plan web
devctl --config "$HOME/.config/devctl/devctl.toml" tui
```

plan 显示磁盘配置，应包含 web 的 npm 命令；它本身不更新现存后台。进入新 TUI 后选中 web 按 s 启动。这里显式指定路径，避免默认发现选中另一份配置。

若新后台仍出现 pnpm 错误，在独立终端运行以下命令，检查 npm 脚本是否间接调用 pnpm：

```sh
cd "$HOME/code/deepmdt-frontend-pc"
npm pkg get scripts packageManager
npm run dev
```

`ERR_PNPM_IGNORED_BUILDS` 表示依赖安装中的构建脚本未获允许；用户日志中涉及 core-js 和 esbuild。它说明执行链进入了 pnpm install，但仅凭这段输出不能确定最外层命令是 npm 还是 pnpm。如果项目确实需要 pnpm，按项目约定在独立终端审核并执行 [pnpm approve-builds](https://pnpm.io/cli/approve-builds)，再重试。不要仅为消除错误批量放行全部依赖；TUI 不转发服务 stdin，交互式审批应在普通终端完成。

若遇到问题，保留实际命令和完整错误，连同以下环境信息一起排查：

```sh
uname -m
uname -r
cat /etc/os-release
rustc --version
devctl --version
command -v node npm pnpm
```

目标 WSL2 上的自动检查可在源码目录运行 `cargo +stable test --locked --test tui_cli` 和 `cargo +stable test --locked`；这些测试使用独立临时配置及 Python 3、sh，不使用上述三个真实项目。剪贴板测试使用假 clip.exe 或截获 OSC 52，不写入真实 Windows 剪贴板。

### 日志复制后无法粘贴

复制通道规则见 [interaction.md](interaction.md#整行选择与复制)。在运行 devctl 的同一个 WSL 环境检查：

```sh
command -v clip.exe
printf 'devctl-copy-test' | clip.exe
```

在 Windows 记事本粘贴核对测试文本。本次目标环境已确认 /mnt/c/WINDOWS/system32/clip.exe 可执行，且上述文本可以正常粘贴。更新后的 TUI 在本机 WSL 使用该程序，反馈应包含 via clip.exe；如果显示 via OSC 52，则实际使用的是终端通道，需核对是否通过 SSH 运行。clip.exe 缺失、启动失败或超时不会被误报为复制成功，按反馈排查互操作与 PATH 后可保留原选区按 y 重试。
