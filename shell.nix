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
  ]);

in
mkShell {
  buildInputs = [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
    custom-python
  ] ++ lib.optionals stdenv.isDarwin apple-deps ++ lib.optionals stdenv.isLinux [
    gdb
  ];

  RUST_BACKTRACE = "1";

  LD_LIBRARY_PATH = if stdenv.isLinux then lib.makeLibraryPath [
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    libglvnd
    vulkan-loader # TODO: needed?
  ] else "";
}
