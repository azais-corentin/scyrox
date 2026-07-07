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

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      mkCrate =
        pname:
        craneLib.buildPackage (
          commonArgs
          // {
            inherit pname cargoArtifacts;
            cargoExtraArgs = "-p ${pname}";
          }
        );

      scyroxctl = mkCrate "scyroxctl";
      scyroxd = mkCrate "scyroxd";
    in
    {
      packages.${system} = {
        inherit scyroxctl scyroxd;
        default = scyroxctl;
      };

      devShells.${system}.default = craneLib.devShell {
        packages = with pkgs; [
          pkg-config
          libusb1
          protobuf
          dprint
          nixfmt
        ];

        # Runtime libraries for scyrox-gui (iced) on NixOS.
        LD_LIBRARY_PATH = lib.makeLibraryPath [
          pkgs.libxkbcommon
          pkgs.wayland
          pkgs.libGL
          pkgs.vulkan-loader
          pkgs.libx11
          pkgs.libxcursor
          pkgs.libxi
          pkgs.libxrandr
        ];
      };

      formatter.${system} = pkgs.nixfmt;
    };
}
