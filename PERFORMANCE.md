# Runtime performance review

Sampled 2026-07-11 using the game's optimized development profile on the
native target, then validated in the wasm build with Chrome's Metal-backed
ANGLE renderer. Browser throughput was sampled uncapped so it measures frame
capacity rather than the display's vsync limit.

## Targeted samples

| Runtime path | Sample | Interpretation |
| --- | ---: | --- |
| Two trajectory traces, default 600-tick horizon | 0.62 ms | Material but acceptable if not repeated every render frame. |
| Two trajectory traces, maximum 360,000-tick horizon / 3,600 samples | 3.73 ms | Dominant CPU system; likely to miss frame budget on wasm when combined with rendering. |
| One relativistic gravity evaluation across the generated system | 0.351 us | Cheap individually; expensive because trajectory invokes it thousands of times. |
| One future collision query | 0.098 us | Not a standalone concern. |
| Advancing the complete orbital hierarchy | 0.790 us/frame | Keep on CPU. |
| Old terrain Gizmo path | 1,696 segments rebuilt/frame | Avoidable CPU geometry generation and upload pressure. |

## Changes made

- Trajectory integration now runs at a 10 Hz navigation cadence and caches the
  result, with immediate invalidation for control-state, horizon, and large
  position changes. At 60 Hz this reduces integration cost by about 83%:
  roughly 0.10 ms/frame amortized at the default horizon and 0.62 ms/frame at
  the maximum horizon in the sampled profile.
- Maximum-horizon trace rendering no longer submits polyline segments. A
  zoom-aware Ramer-Douglas-Peucker pass reduces the simulation samples to at
  most 192 curvature-preserving knots. Persistent triangle meshes carry cubic
  Hermite control data; the vertex shader evaluates the smooth curve and
  expands it into a constant-screen-width ribbon, while the fragment shader
  supplies antialiasing, glow, fading, and active-input dashes. Meshes update
  only when the 10 Hz prediction cache or visual level of detail changes.
- Planet cores and irregular terrain shells are persistent `Mesh2d` assets.
  Vertices and indices are uploaded once; WGPU's vertex pipeline moves them via
  body transforms. Only orbit and landing-zone overlays remain dynamic Gizmos.
- The Bevy-to-Dioxus snapshot bridge now publishes at the same 20 Hz cadence at
  which the DOM UI consumes snapshots, instead of copying fixed-size maps,
  modules, and ship data every render frame.
- The gravity field is evaluated at the vertices of a 48 x 48 tessellated grid
  and interpolated by the fragment shader. This removes the former multi-body
  gravity loop from every pixel while preserving smooth heat-map output. The
  shader also replaces fractional `pow` with reciprocal-square-root
  multiplication and skips body-uniform population when the overlay is off.
- The web canvas backing resolution is synchronized to its CSS dimensions. It
  previously rendered at 1,293 x 1,263 pixels inside a 647 x 631 CSS viewport,
  doing approximately four times the necessary fragment work.
- DOM canvas-size polling runs at 10 Hz, and the Bevy-to-panel-kit telemetry
  bridge publishes at 20 Hz instead of crossing the wasm/DOM boundary every
  frame.

## Browser validation

The earlier uncapped browser table was withdrawn after reproduction showed the
old launch state hit terrain during the test sequence. Later samples were
therefore measuring the game-over screen rather than a live maximum-horizon
simulation. Launch now uses a terrain-clearing circular orbital state and the
physics integrator bounds renderer hitches; subsequent performance sampling
must use that stable live orbit for the complete sequence.

## GPU placement decisions

| Work | Placement | Reason |
| --- | --- | --- |
| Gravity field | Tessellated vertex shader | Multi-body field evaluation is smooth enough at grid vertices and substantially cheaper than evaluating it for every pixel. |
| Adaptive grid lines and heat-map coloring | Fragment shader | Antialiased derivatives and interpolated field coloring are natural fragment work. |
| Trajectory visualization | Adaptive CPU knots + vertex/fragment shader | Collision truth stays in the CPU simulation, while smooth interpolation, ribbon expansion, antialiasing, and dashes are GPU work. |
| Planet terrain geometry | WGPU vertex pipeline | Static irregular geometry should be uploaded once and transformed on GPU. |
| Trajectory integration | Cached CPU simulation | Each state depends on the previous step. A vertex/fragment shader is the wrong execution model, and the web build currently targets WebGL2, which has no compute shaders. A future WebGPU build could use a compute pass. |
| Orbital hierarchy and rigid-body integration | CPU | Stateful ECS simulation is cheap and feeds collision/gameplay logic. |
| Panel-kit UI and minimap | DOM | Interactive text/layout belongs in Dioxus; its update cadence is throttled instead. |

The existing debug build retains Bevy frame-time diagnostics. A future WebGPU
backend should add timestamp queries and a compute trajectory prototype before
moving any additional simulation off the CPU.
