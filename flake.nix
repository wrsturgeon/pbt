{
  inputs = {
    advisory-db = {
      url = "github:rustsec/advisory-db/main?shallow=1";
      flake = false;
    };
    crane-src.url = "github:ipetkov/crane/master?shallow=1";
    flake-utils.url = "github:numtide/flake-utils?shallow=1";
    nixpkgs.url = "github:nixos/nixpkgs/master?shallow=1";
    rust-overlay = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:oxalica/rust-overlay/master?shallow=1";
    };
    treefmt-nix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = "github:numtide/treefmt-nix/main?shallow=1";
    };
  };
  outputs =
    {
      advisory-db,
      crane-src,
      flake-utils,
      nixpkgs,
      rust-overlay,
      self,
      treefmt-nix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowBroken = true;
          overlays = [ (import rust-overlay) ];
        };
        treefmt = treefmt-nix.lib.evalModule pkgs ./.treefmt.nix;
        rust-toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        crane = (crane-src.mkLib pkgs).overrideToolchain (_: rust-toolchain);
        patch-src = raw-src: crane.cleanCargoSource raw-src;
        common-args-from-src =
          raw-src:
          let
            pre = {
              cargoExtraArgs = "--offline";
              cargoLock = ./Cargo.lock;
              src = patch-src raw-src;
              strictDeps = true;
            };
          in
          pre // { cargoArtifacts = crane.buildDepsOnly pre; };
        subprojects =
          let
            filetypes = builtins.readDir ./.;
            directories = builtins.filter (k: filetypes."${k}" == "directory") (builtins.attrNames filetypes);
            with-cargo-toml = builtins.filter (
              directory: (builtins.readDir "${./.}/${directory}") ? "Cargo.toml"
            ) directories;
          in
          with-cargo-toml;
        for-each-subproject =
          f:
          builtins.listToAttrs (
            builtins.map (name: {
              inherit name;
              value = f name;
            }) subprojects
          );
        checks =
          # TODO: `miri` here instead of separately
          let
            by-subproject = for-each-subproject (
              subproject:
              let
                common-args = common-args-from-src "${./.}/${subproject}";
                args = extra: common-args // extra;
              in
              {
                audit = crane.cargoAudit (args {
                  inherit advisory-db;
                });
                build = crane.buildPackage (args { });
                clippy-all-features = crane.cargoClippy (args {
                  cargoClippyExtraArgs = "--all-features --all-targets -- --deny warnings";
                });
                clippy-no-features = crane.cargoClippy (args {
                  cargoClippyExtraArgs = "--no-default-features --all-targets -- --deny warnings";
                });
                deny = crane.cargoDeny (
                  builtins.removeAttrs (args { cargoDenyChecks = "bans licenses sources -c ${./deny.toml}"; }) [
                    "cargoExtraArgs"
                  ]
                );
                doc = crane.cargoDoc (args {
                  env.RUSTDOCFLAGS = "--deny warnings";
                });
                doctest-all-features = crane.cargoDocTest (args {
                  cargoTestExtraArgs = "--all-features";
                });
                doctest-no-features = crane.cargoDocTest (args {
                  cargoTestExtraArgs = "--no-default-features";
                });
                nextest-all-features = crane.cargoNextest (args {
                  cargoNextestExtraArgs = "--all-features";
                  cargoNextestPartitionsExtraArgs = "--no-tests=pass";
                });
                nextest-no-features = crane.cargoNextest (args {
                  cargoNextestExtraArgs = "--no-default-features";
                  cargoNextestPartitionsExtraArgs = "--no-tests=pass";
                });
                style = treefmt.config.build.check self;
              }
            );
            named = builtins.attrValues (
              builtins.mapAttrs (
                subproject: checks:
                builtins.attrValues (
                  builtins.mapAttrs (name: value: {
                    inherit value;
                    name = "${subproject}-${name}";
                  }) checks
                )
              ) by-subproject
            );
            concatenated = builtins.concatLists named;
          in
          builtins.listToAttrs concatenated;
        apps =
          builtins.mapAttrs
            (k: v: {
              type = "app";
              program =
                let
                  script = ''
                    shopt -s nullglob
                    set -euxo pipefail

                    ${v}
                  '';
                  written = pkgs.writeShellScriptBin k script;
                in
                "${written}/bin/${k}";
            })
            {
              ci = ''
                nix flake check --all-systems
                ${rust-toolchain}/bin/cargo miri test --no-default-features
                ${rust-toolchain}/bin/cargo miri test --all-features
              '';
            };
      in
      {
        inherit apps checks;
        devShells.default = pkgs.mkShell {
          inputsFrom =
            (builtins.attrValues self.packages."${system}") ++ (builtins.attrValues self.checks."${system}");
        };
        formatter = treefmt.config.build.wrapper;
        packages = for-each-subproject (
          subproject: crane.buildPackage (common-args-from-src "${./.}/${subproject}")
        );
      }
    );
}
