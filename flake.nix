{
  description = "Scyrox gaming mouse configuration tools";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      crane,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      inherit (pkgs) lib;

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
        ];
      };
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      # crane's default source cleaner drops non-cargo files; keep .proto
      # files so scyrox-proto's build.rs can compile them.
      src = lib.cleanSourceWith {
        src = ./.;
        filter = path: type: (lib.hasSuffix ".proto" path) || (craneLib.filterCargoSources path type);
        name = "source";
      };

      commonArgs = {
        inherit src;
        strictDeps = true;
        pname = "scyrox-workspace";
        version = (lib.importTOML ./Cargo.toml).workspace.package.version;

        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.protobuf
        ];
        buildInputs = [
          pkgs.libusb1
        ];

        # Integration tests require physical hardware; test via `cargo test`
        # in the dev shell instead.
        doCheck = false;
      };

      mkCrateWith =
        pname: extraArgs:
        let
          crateArgs =
            commonArgs
            // extraArgs
            // {
              inherit pname;
              cargoExtraArgs = "-p ${pname}";
            };
          # buildDepsOnly must see the same package selection and native/system
          # dependencies, but not package post-install hooks such as wrapProgram.
          depsArgs = builtins.removeAttrs crateArgs [ "postInstall" ];
        in
        craneLib.buildPackage (
          crateArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly depsArgs;
          }
        );

      mkCrate = pname: mkCrateWith pname { };

      desktopRuntimeLibs = with pkgs; [
        libxkbcommon
        wayland
        libGL
        vulkan-loader
        libx11
        libxcursor
        libxi
        libxrandr
        libayatana-appindicator
      ];

      desktopBuildInputs = desktopRuntimeLibs ++ [
        pkgs.gtk3
      ];

      mkDesktopCrate =
        pname:
        mkCrateWith pname {
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
            pkgs.makeWrapper
          ];
          buildInputs = commonArgs.buildInputs ++ desktopBuildInputs;
          postInstall = ''
            wrapProgram "$out/bin/${pname}" \
              --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath desktopRuntimeLibs}
          '';
        };

      scyroxctl = mkCrate "scyroxctl";
      scyroxd = mkCrate "scyroxd";
      scyrox-tray = mkDesktopCrate "scyrox-tray";
      scyrox-gui = mkDesktopCrate "scyrox-gui";
    in
    {
      packages.${system} = {
        inherit
          scyroxctl
          scyroxd
          scyrox-tray
          scyrox-gui
          ;
        default = scyroxctl;
      };

      devShells.${system}.default = craneLib.devShell {
        packages = with pkgs; [
          pkg-config
          libusb1
          protobuf
          dprint
          nixfmt
          # gtk3 build headers for scyrox-tray (tray-icon gtk feature + tao).
          gtk3
        ];

        # Runtime libraries for scyrox-gui (iced) and scyrox-tray
        # (libayatana-appindicator3.so.1 is dlopened by tray-icon) on NixOS.
        LD_LIBRARY_PATH = lib.makeLibraryPath desktopRuntimeLibs;
      };

      formatter.${system} = pkgs.nixfmt;
    };
}
