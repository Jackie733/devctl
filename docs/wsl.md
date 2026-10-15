# WSL2 安装与项目配置

目标环境已确认是 Windows WSL2；发行版与 CPU 架构尚待核实。当前仓库在 Linux 上通过受控验证，下面是目标 WSL2 的安装与试用流程，不代表已在用户 WSL2 上完成验收。实际进度见 [tasks.md](tasks.md)。

## 1. 在 WSL2 准备源码与工具

在 Windows PowerShell 中可用以下命令核对发行版和 WSL 版本：

```powershell
wsl -l -v
```

其余命令均在目标 WSL2 的 Linux 终端执行。编译时沿用目标环境的 CPU 架构与系统库，运行的是 Linux 二进制。当前实现依赖 Unix socket、进程组、信号及 /proc，Windows 原生 exe 不属于当前目标。

源码建议放在 WSL 的 `~/code/devctl`，三个项目也使用已确认的 WSL 路径。微软建议使用 Linux 工具开发时将项目放在 WSL 文件系统中；参考 [WSL 文件系统说明](https://learn.microsoft.com/en-us/windows/wsl/filesystems) 与 [WSL 基本命令](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)。

确认源码包含 TUI 提交 `feaa688` 或其后续提交。本会话没有自动推送；仅克隆远程仓库不保证拿到本地尚未推送的代码。可将最新源码传到 WSL，或自行推送后在 WSL 获取，不需要复制 target 编译缓存。

本会话提供的源码归档带有提交号，解压后是 devctl 目录，不含 .git 或 target；使用归档时跳过下面的 git log。可以先解压到独立目录，避免覆盖已有源码，再将 devctl 目录放到 ~/code/devctl。

```sh
cd "$HOME/code/devctl"
git log -3 --oneline
rustc --version
cargo --version
node --version
npm --version
pnpm --version
command -v node npm pnpm
```

Node.js、npm 和 pnpm 使用目标 WSL 中安装的 Linux 工具，Node 版本沿用各项目要求；启动命令直接继承运行 devctl 时的 PATH，不通过交互式 shell 加载别名。确认三个命令能在各自项目目录手动运行。

尚未安装 Rust 时，使用 [Rust 官方安装说明](https://rust-lang.org/tools/install/) 安装 rustup 和当前 stable 工具链；已有 rustup 可执行 `rustup update stable`。仓库目前以 Rust 1.98.1 实测，不把运行锁所需的 1.89 当作整个依赖树的最低版本承诺。Ubuntu/Debian 缺少构建工具时可先执行：

```sh
sudo apt update
sudo apt install -y build-essential curl git python3
```

## 2. 编译安装 release

在最新源码目录执行：

```sh
cargo +stable install --path . --locked
"$HOME/.cargo/bin/devctl" --version
```

`cargo install` 默认使用 release 优化构建并安装；通常安装到 `~/.cargo/bin/devctl`，无需先单独运行 `cargo build --release`。`--locked` 使用仓库已有 Cargo.lock；首次安装不加 `--offline`，允许下载依赖。若设置了 CARGO_HOME、CARGO_INSTALL_ROOT 或 Cargo 安装配置，安装目录按其设置变化，以 Cargo 输出为准。参考 [Cargo install 文档](https://doc.rust-lang.org/cargo/commands/cargo-install.html)。

若 `devctl --version` 提示找不到命令，但绝对路径可以运行，在当前终端执行：

```sh
export PATH="$HOME/.cargo/bin:$PATH"
devctl --version
```

需要长期保留时，将该 PATH 配置加入所用 shell 的启动文件。安装完成后直接使用 devctl，不再需要 cargo run。只需要构建文件时，可用 `cargo +stable build --release --locked`，产物为 `target/release/devctl`。

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
cmd = ["pnpm", "run", "dev"]
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

## 4. 校验与进入

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" plan
devctl --config "$HOME/.config/devctl/devctl.toml" tui
```

这里的配置目录只是放置约定，程序不会自动搜索它；从任意目录操作时都传入同一个 `--config` 路径。validate 校验字段、路径与引用，不执行 npm/pnpm，也不保证开发命令或项目依赖可用。

按 [交互方案](interaction.md) 验收：先只启动 web，再启动全部；检查日志、单项停止和重新启动；离开后重新连接；最后全部停止、结束工作区。客户端关闭后的保留行为也需在目标 WSL2 中实测。WSL 的 `wsl --shutdown` / `wsl --terminate` 会终止对应运行环境，后台运行不提供跨 WSL 关闭或 Windows 重启的恢复保证。

## 5. 更新与排查

更新 devctl 二进制或变更配置/Node 环境前，先结束使用该配置的后台；新客户端不会替换仍在运行的旧后台或重新加载其配置和环境。

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" shutdown
```

更新源码后，在源码目录重新执行 `cargo +stable install --path . --locked`，再校验并进入 TUI。后台本就不存在时，shutdown 报无法连接；以当前状态为准，不据此创建后台。

若遇到问题，保留实际命令和完整错误，连同以下环境信息一起排查：

```sh
uname -m
uname -r
cat /etc/os-release
rustc --version
devctl --version
command -v node npm pnpm
```

目标 WSL2 上的自动检查可在源码目录运行 `cargo +stable test --locked --test tui_cli` 和 `cargo +stable test --locked`；这些测试使用独立临时配置及 Python 3、sh，不使用上述三个真实项目。
