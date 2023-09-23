{ pkgs ? import <nixpkgs> { } }:
with pkgs;
let
  apple-frameworks = with darwin.apple_sdk.frameworks; [
    OpenGL
    CoreServices
    AppKit
    Cocoa
    Kernel
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
    go_1_21
    custom-python
  ] ++ lib.optionals stdenv.isDarwin apple-deps ++ lib.optionals stdenv.isLinux [
    gdb
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXinerama
    xorg.libXrandr
    xorg.libXxf86vm
    pkg-config
    libglvnd
  ];

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
