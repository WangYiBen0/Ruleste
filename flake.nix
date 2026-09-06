{
  description = "Ruleste — a from-scratch Rust reimplementation of Celeste using the official itch.io assets (SDL3, no game engine frameworks).";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      crane,
    }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forEachSystem = f: lib.genAttrs systems f;

      # Pinned Rust toolchain via fenix, including the wasm32-unknown-unknown
      # target required to build the Wasm entity plugins.
      toolchain =
        system:
        fenix.packages.${system}.combine [
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
      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell (
            {
              name = "ruleste";

              packages =
                with pkgs;
                [
                  (toolchain system)
                  pkg-config
                  sdl3
                  dotnet-sdk
                  ilspycmd
                  mono
                  lld
                ]
                ++ lib.optionals pkgs.stdenv.isLinux [
                  file
                ];

              # sdl3-sys / pkg-config (sdl3.pc ships in the .dev output)
              PKG_CONFIG_PATH = "${pkgs.sdl3.dev}/lib/pkgconfig";
              LIBRARY_PATH = "${pkgs.sdl3}/lib";
            }
            // lib.optionalAttrs pkgs.stdenv.isLinux {
              LD_LIBRARY_PATH = "${pkgs.sdl3}/lib";
            }
            // lib.optionalAttrs pkgs.stdenv.isDarwin {
              DYLD_LIBRARY_PATH = "${pkgs.sdl3}/lib";
            }
          );
        }
      );

      packages = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = (crane.mkLib pkgs).overrideToolchain (toolchain system);
          # cleanCargoSource respects .gitignore, keeping the 3.4 GB
          # references/ tree out of the build inputs.
          src = craneLib.cleanCargoSource ./.;
          commonArgs = {
            pname = "ruleste";
            version = "0.1.0";
            src = src;
            strictDeps = true;
            buildInputs = [
              pkgs.sdl3
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin (
              with pkgs.darwin.apple_sdk.frameworks;
              [
                Cocoa
                CoreVideo
                Metal
                MetalKit
                ForceFeedback
                IOKit
              ]
            );
            nativeBuildInputs = [ pkgs.pkg-config ];
            doCheck = false;
          };
          # Host binary: `ruleste` game + `ruleste-inspect` asset tool.
          host = craneLib.buildPackage (
            commonArgs
            // {
              cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            }
          );
          # Wasm plugins: player + spring (all non-wall entities).
          wasm = craneLib.buildPackage (
            commonArgs
            // {
              pname = "ruleste-plugins";
              CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
              cargoExtraArgs = "-p ruleste-plugin-player -p ruleste-plugin-spring";
              cargoArtifacts = craneLib.buildDepsOnly (
                commonArgs
                // {
                  pname = "ruleste-plugins";
                  CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
                  cargoExtraArgs = "-p ruleste-plugin-player -p ruleste-plugin-spring";
                }
              );
              installPhase = ''
                mkdir -p $out/share/ruleste/plugins
                cp target/wasm32-unknown-unknown/release/ruleste_plugin_player.wasm \
                  $out/share/ruleste/plugins/ruleste_plugin_player.wasm
                cp target/wasm32-unknown-unknown/release/ruleste_plugin_spring.wasm \
                  $out/share/ruleste/plugins/ruleste_plugin_spring.wasm
              '';
              doInstallCargoArtifacts = false;
            }
          );
        in
        {
          default = pkgs.symlinkJoin {
            name = "ruleste";
            paths = [
              host
              wasm
            ];
          };
          wasm-plugin = wasm;
        }
      );

      apps = forEachSystem (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/ruleste";
        };
      });
    };
}
