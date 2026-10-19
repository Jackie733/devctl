# devctl identity

Selected by the maintainer: three independent horizontal service lines facing
one vertical control rail. The middle line is longer, giving the composition
one clear direction while the matching upper/lower lines provide balance.
The separated strokes express independently controlled services in one workspace.
The mark is static and does not indicate the current selection or service state.

## Assets

| Use | Vector | Transparent 1024px PNG |
| --- | --- | --- |
| Light background | [SVG master](devctl-mark.svg) | [PNG](devctl-mark.png) |
| Dark background | [SVG](devctl-mark-on-dark.svg) | [PNG](devctl-mark-on-dark.png) |

The light-background SVG owns the geometry; the dark-background SVG and both
PNGs have the same geometry. All four strokes have a 44-unit thickness on a
512-unit square canvas. Short lines are 220 units long; the middle line is 304.
The vertical rail is 260 units high and remains separate from all three lines.
The assets have transparent backgrounds and no added frame or effects.

The [original ImageGen concept](source/control-rail-concept.png) and its
[exact prompt](source/prompt.txt) document the selected reference. The SVG master
rebuilds that reference as precise flat geometry; it is the production asset.
Earlier rejected/unselected proposals remain recoverable in Git history.

## Usage

Always display the complete three-horizontal-line and one-vertical-rail shape,
including the longer middle line and all gaps. Scale proportionally. Do not crop,
remove strokes, replace it with punctuation, or use a one-row abbreviation.
Color may follow the background/theme; the four-stroke structure is invariant.

The TUI uses the complete mark in seven columns and three rows, beside the title.
Common line-drawing characters provide the normal treatment; ASCII mode changes
only the stroke characters, preserving every row and its length/spacing:

```text
---   |
----- |
---   |
```

This is the same complete cell geometry, not an abbreviated companion. It needs
no image protocol, new dependency, Nerd Font, or special terminal configuration.
Themes and `NO_COLOR` preserve its structure. A dialog covers the entire background
mark, never a fragment. If a terminal cannot fit the complete mark plus one line
of resize/exit guidance, only the guidance appears; no partial mark is drawn.
The regular 36 × 10 minimum remains supported.

The English/Chinese READMEs display the same full vector geometry, with background-
appropriate color. Existing CLI help, status and log text keep their text formats;
the brand integration does not insert decorative output into data/diagnostics.

## Terminal preview

[Preview](tui-preview.png) shows the running local build in dark, light, and
monochrome/ASCII modes. It reconstructs actual PTY characters and emitted colors
with Menlo; it is not a native terminal-window screenshot. The capture used
isolated test services and cleaned up their background workspaces afterward.
