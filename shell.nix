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
in
mkShell {
  buildInputs = [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
  ] ++ lib.optionals stdenv.isDarwin apple-deps;
}
