{
  description = "Ruleste — a from-scratch Rust reimplementation of Celeste using the official itch.io assets (SDL3, no game engine frameworks).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
    }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSystem = f: lib.genAttrs systems f;
    in
    {
      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          # Pinned Rust toolchain via fenix, including the wasm32-unknown-unknown
          # target required to build the Wasm entity plugins.
          toolchain = fenix.packages.${system}.combine [
            (fenix.packages.${system}.stable.withComponents [
              "cargo"
              "rustc"
              "rust-src"
              "rust-analyzer"
              "clippy"
            ])
            fenix.packages.${system}.targets.wasm32-unknown-unknown.stable.rust-std
          ];
        in
        {
          default = pkgs.mkShell {
            name = "ruleste";

            packages = [
              toolchain
              pkgs.pkg-config
              # SDL3 for windowing/input/audio abstraction (use-pkg-config).
              pkgs.sdl3
              # Decompilation / format-inspection helpers.
              pkgs.dotnet-sdk
              pkgs.ilspycmd
              pkgs.mono
            ]
            ++ lib.optionals pkgs.stdenv.isLinux [
              pkgs.file
            ];

            # sdl3-sys / pkg-config (sdl3.pc ships in the .dev output)
            PKG_CONFIG_PATH = "${pkgs.sdl3.dev}/lib/pkgconfig";
            LIBRARY_PATH = "${pkgs.sdl3}/lib";
            LD_LIBRARY_PATH = "${pkgs.sdl3}/lib";
          };
        }
      );
    };
}
