# Terminal-first logo directions

The maintainer rejected the initial lowercase `d` direction and requested a more
modern, minimal identity that can also appear in the TUI. These are three new
proposals for that request, not an approved identity or an integrated UI change.

| Direction | Original image | TUI text | Intent |
| --- | --- | --- | --- |
| Gate | [PNG](gate.png) | `>\|` | Execution meeting a control boundary |
| Parallel | [PNG](parallel.png) | `::` | Independent service channels, controlled together |
| Workspace | [PNG](workspace.png) | `[=]` | Parallel services inside one workspace |

The files are independent built-in ImageGen outputs, each 1254 × 1254 RGB with
an opaque background. They are visual concepts, with slight tonal variation,
not exact flat-color vector masters. The exact prompts are saved in
[Gate](gate-prompt.txt), [Parallel](parallel-prompt.txt), and
[Workspace](workspace-prompt.txt). Each generation used a raster reconstruction
of the current TUI's real PTY output as product context, not as a logo to copy.
No font or vector source is embedded in these images.

## TUI presentation proposal

[Terminal specimens](tui-specimens.png) show the corresponding ordinary ASCII
marks on dark, light, and monochrome backgrounds. These are designed text
specimens, not screenshots of an integrated implementation. Parallel's `::`
is a simplified companion, not an exact reproduction of its offset gaps.

Recommend a graphical mark for documentation and a text companion in the existing
one-row TUI header. Use the existing theme accent, remove color under `NO_COLOR`,
and keep the identity static so it cannot be confused with animated service state.
Ordinary ASCII needs neither Nerd Fonts nor an image-capable terminal.

At narrow widths, a future implementation should omit version and workspace ID
before connection information, leaving room for `Disconnected` and unknown-state
feedback. The specimen strings fit 36 cells; they do not define new runtime
behavior. Keep existing status labels, layout height, and help/navigation scope.

| Rendering approach | Suitable placement | Tradeoff |
| --- | --- | --- |
| Ordinary text mark + name | Default one-row header | Most predictable; exact proportions follow the terminal font |
| Unicode or block-cell mark | Optional larger empty/help surface | Closer geometry, but consumes rows and needs glyph-width/fallback checks |
| PNG through a terminal graphics protocol | Optional supported-terminal view | Closest to artwork; needs capability detection, resize/erase handling, and text fallback |

The default recommendation is the first approach. Image support is technically
available through [ratatui-image](https://github.com/ratatui/ratatui-image), which
documents protocol/font-size detection and half-block fallback. Kitty's
[icat documentation](https://sw.kovidgoyal.net/kitty/kittens/icat/) notes that image
support through terminal multiplexers depends on their support. These are design
tradeoffs, not verified compatibility claims for devctl's supported terminals.
No dependency or runtime setting is added by this proposal.

## Validation and remaining work

- `cargo build --locked` passed for the current 1.8.1 source.
- Ran the existing real-PTY fixture in an isolated temporary workspace: service
  start, log output, 120 × 30 to 36 × 10 resize, client exit, and terminal/mouse-mode
  restoration passed; the fixture cleaned up its own background workspace.
- Decoded all three PNGs and inspected them at 16/24/32/64 px. At 16 px, internal
  detail is reduced, especially for Workspace. Prefer 24 px or larger for artwork.
- Inspected text specimens on dark/light/monochrome backgrounds using Menlo;
  all narrow proposal headers fit 36 ASCII cells.
- Checked changed Markdown local file links, fences, and whitespace.

The native Terminal app was unavailable to computer-use capture. The grounding
image was reconstructed from actual PTY text; it is not a native-window screenshot
and does not preserve the original per-cell colors. No modified-logo TUI, actual
Windows Terminal/WSL view, terminal graphics protocol, or special-font behavior
has been tested. The candidate title strings are design specimens only.

Next: choose a direction, create its exact vector master and terminal companion,
then integrate and validate that selected identity. Preserve the old image as a
superseded proposal. No push or publication is included.
