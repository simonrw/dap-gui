let
  overlays = [
    (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
  ];
in

{ pkgs ? import <nixpkgs> { inherit overlays; } }:
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
in
mkShell rec {
  buildInputs = [
    rust-bin.beta.latest.default
    rust-analyzer
    cargo-nextest
    cargo-flamegraph
    cargo-hack
    act
    maturin
    python3Packages.venvShellHook
  ] ++ lib.optionals stdenv.isDarwin apple-deps ++ lib.optionals stdenv.isLinux [
    gdb
    simplescreenrecorder
    cargo-llvm-cov
  ];
  RUST_BACKTRACE = "1";

  RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";

  RUST_LOG = "gui=trace,end_to_end=debug,transport=debug,dap_gui_client=debug,debugger=debug";

  venvDir = ".venv";
  VIRTUAL_ENV = venvDir;

  postVenvCreation = ''
  python -m pip install \
    debugpy \
    pytest \
    ipython
  '';

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
