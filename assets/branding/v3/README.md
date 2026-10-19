# Service lines and a control rail

[Concept image](control-rail.png) · [Exact generation prompt](prompt.txt) ·
[Earlier alternatives](../v2/README.md)

The maintainer requested an explainable, simple, attractive symbol after the
previous alternatives were presented without sufficient design rationale.
This proposal starts from devctl's confirmed use case: independent local services
managed through one workspace. It is a new exploration, not an approved identity.

## Meaning

Three separate horizontal strokes stand for multiple independent services. Three
is a compact visual shorthand for a group, not a limit on project count. One
vertical stroke stands for the shared control surface. The strokes do not connect:
the symbol does not depict a dependency graph or imply runtime orchestration.
The longer middle stroke draws the eye toward the control rail; it is a static
compositional emphasis, not a live selected/running-service indicator.

## Form

Only four strokes are needed. A shared left edge and equally spaced horizontal
rows establish order. Matching short top/bottom strokes provide balance; the
longer middle stroke introduces direction. The isolated vertical rail gives the
composition an endpoint, with open space instead of an enclosing frame.

The identity is judged in monochrome first. Stroke weight and spacing, rather
than color effects or extra symbols, should carry recognition. Beauty remains
a design judgment; these choices explain the intended balance without claiming
that the result has been accepted or is uniquely identifiable in the market.

The earlier `>|` has a clear execution cue but resembles a media control; `::`
is compact but too ambiguous; `[=]` expresses containment but gives the enclosure
too much weight. This concept preserves parallel services and one control point
while opening the composition.

## TUI adaptation

The complete mark can be redrawn with terminal cells on a larger help/empty
surface. A one-line title can use a simplified ASCII `=| devctl` companion.
It preserves horizontal strokes facing a vertical rail, not the exact three-line
silhouette. It requires no special font or terminal graphics protocol.

The closer Unicode `≡│` is a candidate only: both characters have East Asian
ambiguous width, and the direct font rasterizer did not show both correctly in
SF Mono. Do not claim universal rendering or choose it as the default without
actual-terminal checks. No TUI implementation is included in this design pass.

## Asset and checks

Created with built-in ImageGen; the original PNG is 1254 × 1254 RGB on an opaque
pale background. It has slight tonal variation and is not an exact vector master.
The prompt specifies target geometry, not measured output coordinates.

Decoded the file and inspected 16/24/32/48/64 px downscales. Prefer at least 24 px;
16 px loses separation and detail. Inspected terminal companions with Menlo and
SF Mono, recording the Unicode limitation above. Checked changed Markdown links,
code fences and whitespace. No Rust source or runtime behavior changed.

Next: visual feedback on the meaning and composition; then an exact geometric
master, exports and selected TUI treatment. Previous alternatives remain
available for comparison. No publication or push is included.
