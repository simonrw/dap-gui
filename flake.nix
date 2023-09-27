{
  description = "Flake utils demo";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [
          (import rust-overlay)
        ];

        pkgs = import nixpkgs {
          inherit overlays system;
        };
      in
      {
        devShells = rec {
          default =
            with pkgs;
            let
              apple-frameworks = with darwin.apple_sdk.frameworks; [
                OpenGL
                CoreServices
                AppKit
              ];
              apple-libs = [
                libiconv
              ];

              apple-deps = apple-frameworks ++ apple-libs;

              custom-python = python3.withPackages (ps: with ps; [
                debugpy
                black
                scapy
                structlog
              ]);

            in
            mkShell {
              buildInputs = [
                (rust-bin.beta.latest.default.override {
                  extensions = [ "rust-src" ];
                })
                rust-bin.beta.latest.rust-analyzer
                cargo-nextest
                custom-python
              ] ++ lib.optionals stdenv.isDarwin apple-deps ++ lib.optionals stdenv.isLinux [
                gdb
              ];

              RUST_BACKTRACE = "1";

              RUST_SRC_PATH = "${rust-bin.beta.latest.rust-src}";

              RUST_LOG = "dap_gui=trace,end_to_end=debug,dap_gui_ui=debug,dap_gui_client=debug";

              LD_LIBRARY_PATH =
                if stdenv.isLinux then
                  lib.makeLibraryPath [
                    xorg.libX11
                    xorg.libXcursor
                    xorg.libXrandr
                    xorg.libXi
                    libglvnd
                    vulkan-loader # TODO: needed?
                  ] else "";
            };
        };
      }
    );
}
