{
  inputs = {
    advisory-db = {
      flake = false;
      url = "github:rustsec/advisory-db";
    };
    crane-flake.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    treefmt-nix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:numtide/treefmt-nix/main";
    };
  };
  outputs =
    {
      advisory-db,
      crane-flake,
      flake-utils,
      nixpkgs,
      self,
      treefmt-nix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        crane = crane-flake.mkLib pkgs;

        src = crane.cleanCargoSource ./.;

        craneDepArgs = { inherit src; };
        cargoArtifacts = crane.buildDepsOnly craneDepArgs;
        craneArgs = craneDepArgs // {
          inherit cargoArtifacts;
        };
        craneArtifacts = crane.buildPackage (craneArgs // { doCheck = false; });

        treefmt = treefmt-nix.lib.evalModule pkgs ./.treefmt.nix;
      in
      {
        checks = {
          audit = crane.cargoAudit { inherit src advisory-db; };
          build = craneArtifacts;
          clippy = crane.cargoClippy (
            craneArgs // { cargoClippyExtraArgs = "--all-targets -- --deny warnings"; }
          );
          deny = crane.cargoDeny craneArgs;
          doc = crane.cargoDoc (craneArgs // { env.RUSTDOCFLAGS = "--deny warnings"; });
          fmt-rust = crane.cargoFmt craneArgs;
          fmt-toml = crane.taploFmt craneArgs;
          tests = crane.cargoNextest (craneArgs // { cargoNextestPartitionsExtraArgs = "--no-tests=pass"; });
        };
        devShells.default = crane.devShell { checks = self.checks.${system}; };
        formatter = treefmt.config.build.wrapper;
        packages.default = craneArtifacts;
      }
    );
}
