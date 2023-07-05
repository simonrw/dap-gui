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

  debugpy-server = python3.withPackages (ps: with ps; [
    debugpy
  ]);

in
mkShell {
  buildInputs = [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
    debugpy-server
    gdb
  ] ++ lib.optionals stdenv.isDarwin apple-deps;

  RUST_BACKTRACE = "1";

  LD_LIBRARY_PATH = if stdenv.isLinux then lib.makeLibraryPath [
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    vulkan-loader # TODO: needed?
  ] else "";
}
