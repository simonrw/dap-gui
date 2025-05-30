{pkgs, ...}:
with pkgs; let
  apple-frameworks = with darwin.apple_sdk.frameworks; [
    OpenGL
    CoreServices
    AppKit
  ];
  apple-libs = [libiconv];

  apple-deps = apple-frameworks ++ apple-libs;

  custom-python = python3.withPackages (ps:
    with ps; [
      debugpy
      black
      scapy
      structlog
      ruff
    ]);

  toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
in
  mkShell rec {
    buildInputs =
      [
        act
        bacon
        cargo-flamegraph
        cargo-hack
        cargo-nextest
        custom-python
        delve
        maturin
        pyright
        toolchain
        uv
        ruff
      ]
      ++ lib.optionals stdenv.isDarwin apple-deps
      ++ lib.optionals stdenv.isLinux [
        gdb
        simplescreenrecorder
        cargo-llvm-cov
      ];

    env = {
      RUST_BACKTRACE = "1";
      RUST_LOG = "gui=trace,end_to_end=debug,transport=debug,dap_gui_client=debug,debugger=debug,pythondap=debug,tui=debug,repl=debug";
      RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
    };

    postVenvCreation = ''
      python -m pip install \
        debugpy \
        pytest \
        ipython
    '';

    LD_LIBRARY_PATH =
      if stdenv.isLinux
      then
        lib.makeLibraryPath [
          libxkbcommon
          xorg.libX11
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          libglvnd
          vulkan-loader # TODO: needed?
        ]
      else "";
  }
