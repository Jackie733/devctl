# devctl icon

Superseded proposal: the maintainer rejected this lowercase `d` direction.
See the [terminal-first alternatives](v2/README.md) for the current exploration.

[Original PNG](devctl-icon.png) — 1254 × 1254 pixels, RGBA, with transparent
space outside the dark rounded tile.

The mark combines a lowercase **d** with the terminal prompt **>**. The enclosing
shape suggests a single place to control development services. Mint teal against
deep blue-black reflects a calm, focused terminal tool.

This is a superseded visual proposal. It is not
installed into the terminal interface or release packaging. The PNG is the
original image-generation output; no editable vector master is included.

## Usage and inspection

Use at 24 pixels or larger when both the letter and terminal prompt must be clear.
At 16 pixels the silhouette remains visible, but the internal detail becomes less
distinct. Avoid adding shadows, extra symbols, or text inside the tile.

Inspected on light and dark backgrounds at 16, 24, 32, 48, and 64 pixels. Checked
square dimensions, the alpha range, and transparency at all four outer corners.
These checks verify the supplied asset; they do not establish maintainer approval.

## Generation

Created with the built-in Codex ImageGen tool. The prompt requested a flat,
vector-like appearance; the result is a raster image with slight tonal variation.
The requested palette was mint teal `#52E0BC` and deep blue-black `#111820`;
these values describe the design intent rather than exact colors of every pixel.

Exact generation prompt:

```text
Use case: logo-brand.
Create ONE final polished logo / application icon for "devctl", a lightweight Rust terminal workspace manager for local development services. Developers start/stop multiple projects in one place, inspect logs, leave the terminal while services continue in background, reconnect later. Personality: precise, calm, lightweight, dependable, modern developer tooling.
Asset: a single square icon, no wordmark, no presentation board, no multiple versions.
Design a distinctive bold geometric lowercase d / terminal-control symbol: a thick open rounded-square loop forms an abstract lowercase d silhouette, with a simple right-pointing terminal chevron integrated into its interior through deliberate negative space. The result must look like one coherent custom mark, not a collection of pictograms. Large open counters, even substantial weight, optical balance. A subtle top opening or cut suggests ongoing service/control, but do NOT use a generic power button.
Composition: centered on a flat near-black blue rounded-square tile (#111820), generous equal padding of about 22%, mark occupies about 58% of tile. Rounded tile corners about 22% of width. Logo mark in solid fresh mint-teal (#52E0BC). Use only those two flat colors, with genuinely transparent space OUTSIDE the rounded tile.
Style: expertly designed flat vector-like identity, extremely clean crisp edges, strong simple silhouette, balanced at 24px, restrained and timeless, slightly softened corners.
Avoid: text, letters spelling the name, tagline, 3D, bevel, glow, gradients, reflections, drop shadows, texture, terminal-window frame, tiny dots, circuit networks, gears, mascots, decorative borders, busy lines, mockups. Deliver only the finished isolated icon at 1024x1024.
```
