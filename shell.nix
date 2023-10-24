{ pkgs ? import <nixpkgs> { } }:
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
    cargo
    rustc
    rust-analyzer
    rustfmt
    clippy
    cargo-nextest
    cargo-flamegraph
    custom-python
    cargo-hack
    act
  ] ++ lib.optionals stdenv.isDarwin apple-deps ++ lib.optionals stdenv.isLinux [
    gdb
    simplescreenrecorder
    cargo-llvm-cov
  ];
  RUST_BACKTRACE = "1";

  RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";

  RUST_LOG = "gui=trace,end_to_end=debug,transport=debug,dap_gui_client=debug";

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
}
