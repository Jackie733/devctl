# black-prism 启动动画预览

对应 U01。用户在 Alacritty 上拒绝半块预览后，已选择优先还原官方网页效果。本轮改为官方 Canvas 生成 PNG 帧，由 Kitty 图形协议播放；仍是独立预览，没有连接后台或接入正式 Rust TUI。

## 像素预览

本会话已经生成 `target/splash-pixels`，在 **Kitty 的直接 shell 中，退出 tmux**，从 devctl 仓库执行：

```sh
python3 examples/splash/preview-pixels.py target/splash-pixels
```

固定播放 30 帧、15 FPS、约 2 秒。Enter/Space 跳过，Ctrl-C 返回 130；SIGTERM/SIGHUP、绘制失败或正常结束均尝试删除本次图像、恢复光标/备用屏幕和终端属性。窗口变化时重新放置图片并保持比例。正式操作语义维护在 [interaction.md](../../docs/interaction.md)。

这个预览先检查 TTY、TERM 和 tmux，再发送实际图形能力探测；不支持或请求失败则明确报错。它目前只以 Kitty 为目标，不宣称其他实现 Kitty 图形协议的终端也支持其动画指令。Alacritty 中不能播放此像素版本；其缺少图像显示能力的证据见 [兼容性说明](https://github.com/ratatui/ratatui-image#compatibility-matrix)。

可先在浏览器打开生成的 `target/splash-pixels/reference.html`，查看原 Canvas 渲染器的两秒动效，点击 Replay 重播；这只作为视觉参考，不替代实际终端验收。

## 准备帧

准备工具需要 Git、Node.js 24、Firefox 和 ImageMagick 的 `convert`；没有 npm 依赖。播放器只需要 Python 3 和支持动画的 Kitty。正式 Rust 二进制计划内嵌帧，不要求用户安装这些生成工具。

源码临时目录被清理后，先重新准备；不要覆盖已有 checkout：

```sh
git clone https://github.com/bas3line/ascii.git /tmp/devctl-ascii-reference
git -C /tmp/devctl-ascii-reference checkout --detach a98915a9fa0d0d74c8a2094935a02de4425e9b1d
```

生成到一个尚不存在的目录，例如：

```sh
node examples/splash/prepare-pixels.mjs /tmp/devctl-ascii-reference target/splash-pixels-new
python3 examples/splash/preview-pixels.py target/splash-pixels-new
```

工具检查固定版本及两个源文件未修改。使用 Node 的 TypeScript 类型擦除将上游 black-prism.ts 与 mount.ts 放入本地 HTML；在精确的 t=0、1/15、…、29/15 秒冻结原生成器，让 Firefox 使用原 Canvas 绘制每帧，再无损裁切为 800 × 400 PNG。没有经过半块绘制、GIF 调色板量化或自行重写粒子算法。

输出包含 30 张 PNG、manifest、浏览器参考页、截图中间文件及上游许可证；均在被 Git 忽略的 target 中。源网页字体会随系统而异，终端缩放也可能影响细节，尚不承诺和用户浏览器逐像素一致。

## 已验证与待验证

- Firefox 157.0.1 无头生成成功，30 张 PNG 内容不同；合计 605827 字节。静态检查可见不同大小圆点、光束与倒影。
- 模拟 Kitty 响应的 PTY 检查覆盖正常两秒播放、Space、Enter、Ctrl-C、SIGTERM、SIGHUP、窗口缩放、上传时跳过、拒绝图形请求、探测无响应、Alacritty 和 tmux 拒绝。检查传输 PNG 与源文件完全一致，并检查终端恢复。
- 当前执行环境没有 Kitty 和图形桌面会话；没有真实 Kitty 显示或性能证据。需要用户试播，确认圆点大小、运动、锐度、比例及退出后恢复。

## 上一版预览

`preview.mjs` 保留用于比较。它复用上游 terminal.ts，把 `· / • / ●` 混色、平滑、缩小后转成 `▀` 色块，用户反馈粒子形状和运动不可见。原始帧仍有时间变化；该版本未通过视觉验收，不作为高还原方案。

## 来源

- [black-prism 固定版本源码](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/src/pieces/black-prism.ts)
- [终端播放器](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/src/terminal.ts)
- [MIT 许可证](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/LICENSE)

正式复制或改编源代码、分发派生动画资源时保留来源及上游许可证文本。
