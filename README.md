# Gerbil Space Program

A lunar-lander game built with [Bevy](https://bevyengine.org/). Fly the lander, manage fuel and velocity, and touch down safely on procedurally generated terrain.

## Run locally

Native:

```sh
cargo run
```

Web:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve
```

The web build is served at `http://localhost:8080` by default.

## Controls

- Use the configured thrust controls to maneuver the lander.
- Pause from the in-game pause control.
- Simulation options, including gravity, thrust, time scaling, trajectory display, and infinite fuel, are available from Settings.

## Platforms

The project targets Windows, Linux, macOS, WebAssembly, Android, and iOS. Mobile build configuration lives in [`mobile/`](mobile/).

## Credits and license

Third-party asset credits are recorded in [`credits/CREDITS.md`](credits/CREDITS.md).

The project is licensed under [CC0 1.0 Universal](LICENSE), except for content noted in the credits.
