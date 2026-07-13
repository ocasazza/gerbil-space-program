{
  description = "Gerbil Space Program — Bevy game dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    {
      devShells = forAllSystems (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };

          # Bevy 0.16's dependency graph is built around the Rust 1.85-era
          # wasm toolchain. Newer rustc versions can emit duplicate exports
          # with the locked wasm-bindgen 0.2.99, producing invalid modules.
          rustToolchain = pkgs.rust-bin.stable."1.85.0".default.override {
            targets = [ "wasm32-unknown-unknown" ];
          };

          linuxBuildInputs = with pkgs; [
            alsa-lib-with-plugins
            udev
            vulkan-loader
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            libxkbcommon
            wayland
          ];

          shell = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.trunk
              pkgs.wasm-bindgen-cli
              pkgs.binaryen
              pkgs.lld
              pkgs.pkg-config
            ]
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux linuxBuildInputs;

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
              pkgs.lib.optionals pkgs.stdenv.isLinux linuxBuildInputs
            );

            shellHook = ''
              echo "🦀 Rust: $(rustc --version)"
              echo "🌐 Trunk: $(trunk --version)"
              echo ""
              echo "  trunk serve    — start dev server (http://localhost:8080)"
              echo "  cargo build    — native debug build"
              echo "  cargo run      — run native"
            '';
          };
        in
        { default = shell; }
      );
    };
}
