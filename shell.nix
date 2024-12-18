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
    ]);

  toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
in
  mkShell rec {
    buildInputs =
      [
        toolchain
        rust-analyzer-unwrapped
        cargo-flamegraph
        custom-python
        cargo-hack
        act
        maturin
        uv
      ]
      ++ lib.optionals stdenv.isDarwin apple-deps
      ++ lib.optionals stdenv.isLinux [
        gdb
        simplescreenrecorder
        cargo-llvm-cov
      ];

    env = {
      RUST_BACKTRACE = "1";
      RUST_LOG = "gui=trace,end_to_end=debug,transport=debug,dap_gui_client=debug,debugger=debug,pythondap=debug";
      RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
    };

    shellHook = ''
      export RUST_BUILD_BASE="$HOME/.cache/rust-builds"
      WORKSPACE_ROOT=$(cargo metadata --no-deps --offline 2>/dev/null | jq -r ".workspace_root")
      PACKAGE_BASENAME=$(basename $WORKSPACE_ROOT)
      # Run cargo with target set to $RUST_BUILD_BASE/$PACKAGE_BASENAME
      export CARGO_TARGET_DIR="$RUST_BUILD_BASE/$PACKAGE_BASENAME"
    '';

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
