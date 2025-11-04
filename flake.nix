{
  description = "Omniglot";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";

    treefmt-nix.url = "github:numtide/treefmt-nix";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      treefmt-nix,
      fenix,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        inherit (pkgs) lib;

        rustToolchainComponents = [
          "rustc"
          "cargo"
          "rustfmt"
        ];

        msrvRustToolchain =
          (fenix.packages."${system}".fromToolchainName {
            name = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.rust-version;
            sha256 = "sha256-KUm16pHj+cRedf8vxs/Hd2YWxpOrWZ7UOrwhILdSJBU=";
          }).withComponents
            rustToolchainComponents;

        stableRustToolchain = fenix.packages."${system}".stable.withComponents rustToolchainComponents;
        stableRustPlatform = pkgs.makeRustPlatform {
          rustc = stableRustToolchain;
          cargo = stableRustToolchain;
        };

        rustPackagesForToolchain = rustToolchain: rec {
          craneLib = (crane.mkLib pkgs).overrideToolchain (_p: rustToolchain);

          baseRustBuildArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
          };

          # Build *just* the cargo dependencies (of the entire workspace), so we
          # can reuse all of that work (e.g. via cachix) when running in CI:
          cargoArtifacts = craneLib.buildDepsOnly baseRustBuildArgs;

          omniglot = craneLib.buildPackage (
            baseRustBuildArgs
            // {
              inherit cargoArtifacts;
            }
          );
        };

        treefmt =
          (treefmt-nix.lib.evalModule (pkgs.extend (
            self: super: {
              rustfmt = stableRustToolchain;
            }
          )) ./treefmt.nix).config.build;
      in
      rec {
        packages.default = (rustPackagesForToolchain stableRustToolchain).omniglot;

        # Check formatting and build all packages:
        checks = {
          omniglot-msrv = (rustPackagesForToolchain msrvRustToolchain).omniglot;
          omniglot-stable = (rustPackagesForToolchain stableRustToolchain).omniglot;
          formatting = treefmt.check self;
        };

        formatter = treefmt.wrapper;

        devShells.default = pkgs.mkShell {
          name = "omniglot-devshell";

          packages = with pkgs; [
            stableRustToolchain
          ];
        };
      }
    );
}
