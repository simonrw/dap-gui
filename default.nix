{pkgs ? import <nixpkgs> {}}:
pkgs.rustPlatform.buildRustPackage {
  pname = "dap-gui";
  version = "0.1.0";
  src = pkgs.nix-gitignore.gitignoreSource [] ./.;
  cargoLock = {
    lockFile = ./Cargo.lock;
  };
  buildFeatures = ["sentry"];
  nativeBuildInputs = with pkgs; [
    makeBinaryWrapper
    (python3.withPackages (ps:
      with ps; [
        debugpy
      ]))
    go
    delve
  ];
  postInstall = ''
    wrapProgram $out/bin/gui \
      --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [
        pkgs.libxkbcommon
      ]}
  '';

  doCheck = false;
}
