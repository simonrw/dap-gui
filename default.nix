{ pkgs ? import <nixpkgs> { } }:
pkgs.rustPlatform.buildRustPackage {
  pname = "dap-gui";
  version = "0.1.0";
  src = pkgs.nix-gitignore.gitignoreSource [ ] ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
  buildFeatures = [ "sentry" ];
  nativeBuildInputs = with pkgs; [
    makeBinaryWrapper
    (python3.withPackages (ps:
      with ps; [
        debugpy
      ]))
    go
    delve
  ];
  buildInputs = if pkgs.stdenv.isLinux then [ ] else
  (with pkgs.darwin.apple_sdk.frameworks; [ AppKit ]);
  postInstall =
    if pkgs.stdenv.isLinux then ''
          wrapProgram $out/bin/gui \
            --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
      pkgs.libxkbcommon
      ]}
    '' else "";

  doCheck = false;
}
