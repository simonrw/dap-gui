{ pkgs ? import <nixpkgs> {} }:
with pkgs;
let
  apple-frameworks = with darwin.apple_sdk.frameworks; [
    Metal
    QuartzCore
    AppKit
  ];
in
mkShell {
  buildInputs = [
    go
  ] ++ lib.optionals stdenv.isDarwin apple-frameworks;
}
