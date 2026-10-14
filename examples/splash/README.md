# black-prism 启动动画预览

对应 U01 的启动展示提案。此例只预览动画，不连接后台、不启动服务，也不代表正式 TUI 已实现。先确认终端视觉效果，再生成 Rust 使用的内嵌帧。

预览需要 Git 与 Node.js 24；直接运行上游 TypeScript 源码，无需 npm install。Node.js 只用于本次开发预览，正式 devctl 的启动动画仍计划在 Rust 中播放。

## 运行

本会话已经准备好 `/tmp/devctl-ascii-reference`，从 devctl 仓库执行：

```sh
node examples/splash/preview.mjs /tmp/devctl-ascii-reference
```

默认播放 2 秒、15 FPS。若时间太短，延长观察：

```sh
node examples/splash/preview.mjs /tmp/devctl-ascii-reference --seconds 6
node examples/splash/preview.mjs /tmp/devctl-ascii-reference --mono
```

Enter、Space 或其他按键都跳过这次预览；Ctrl-C 结束并返回 130。预览结束后回到 shell，目前不会进入项目管理界面。输入或输出不是 TTY、初始窗口小于 40 列 × 12 行时跳过。正式键位与行为维护在 [interaction.md](../../docs/interaction.md)。

临时目录被清理后，可在其他路径重新准备源码；不要覆盖已有 checkout：

```sh
git clone https://github.com/bas3line/ascii.git /tmp/devctl-ascii-reference
git -C /tmp/devctl-ascii-reference checkout --detach a98915a9fa0d0d74c8a2094935a02de4425e9b1d
```

预览检查 HEAD 是否等于上述版本，不下载代码；请使用未修改的上游 checkout。源码和播放器保持在独立 checkout 中，本仓库没有复制第三方代码。

## 观察

- 棱镜是否保持三角形比例，白光、彩虹和倒影是否清晰。
- 分别在常用大小、80 × 24 和较窄窗口中查看；播放中调整窗口。
- 正常结束、按键跳过、Ctrl-C 后，原 shell 内容、光标与输入是否恢复。
- 两秒是否足够，是否愿意每次进入 TUI 都播放。

原画面是 200 × 100 个方形采样单元，上游终端播放器进行平滑、等比缩小和上下半块配色，所以不是直接输出 100 行字符。默认保留黑色画面背景和 RGB 色彩；实际颜色表现取决于终端支持。

## 已发现的视觉差异

用户在 Alacritty 上试播后反馈：画面不如官方网页清晰，粒子看起来不移动且尺寸固定。当前预览未通过视觉验收。

对固定版本源码的检查表明，网页 mount.ts 使用 Canvas 逐格绘制 `· / • / ●` 与调色板颜色；terminal.ts 则将圆点覆盖率混入背景，平滑、缩小后绘制 `▀` 色块。因此当前预览没有保留网页上的粒子形状与大小变化。半块绘制仍有动画，但低分辨率和重绘时忽略小幅颜色差异会使运动不明显。

原始动画未冻结：使用同一个帧生成器比较 t=0、0.5、1、2 秒，后续三帧相对上一帧分别有 544、453、787 个字符变化，以及 1007、544、1384 个调色板索引变化。这只证明原始帧随时间变化，不证明用户终端上的运动足够明显。

Alacritty 的常规字符网格无法按网页 Canvas 的像素精度绘制该场景；[ratatui-image 的兼容性说明](https://github.com/ratatui/ratatui-image#compatibility-matrix)也将 Alacritty 的 Sixel 图形支持标为不可用。可继续探索保留圆点字符与颜色的近似渲染；若要求接近网页的像素细节，应先选择并验证支持图像协议的终端方案。更高帧率或延长播放不能恢复已被平滑抹掉的粒子形状。

## 来源

- [black-prism 固定版本源码](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/src/pieces/black-prism.ts)
- [终端播放器](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/src/terminal.ts)
- [MIT 许可证](https://github.com/bas3line/ascii/blob/a98915a9fa0d0d74c8a2094935a02de4425e9b1d/LICENSE)

正式复制或改编源代码、分发派生动画资源时保留来源及上游许可证文本。
