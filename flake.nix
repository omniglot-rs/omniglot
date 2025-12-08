{
  description = "Omniglot";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";

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
            name = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.rust-version;
            sha256 = "sha256-Qxt8XAuaUR2OMdKbN4u8dBJOhSHxS+uS06Wl9+flVEk=";
          }).withComponents
            rustToolchainComponents;

        stableRustToolchain = fenix.packages."${system}".stable.withComponents rustToolchainComponents;

        rustPackagesForRustToolchain = rustToolchain: rec {
          craneLib = (crane.mkLib pkgs).overrideToolchain (_p: rustToolchain);

          cleanedRustSrc =
            let
              # Path comes with "/nix/store/${hash}-source/" stripped
              relSrcFilter =
                relPath: type:
                # Include C header files and compiled artifacts for the `add` example:
                (lib.hasPrefix "examples/add/libadd" relPath);

              # Strip "/nix/store/${hash}-source/" prefix:
              trimStorePathPrefix =
                path: builtins.head (builtins.match "^\/nix\/store\/[a-zA-Z0-9]+\-source\/(.*)" path);

              # Combine crane's Cargo source filter and a custom one operating on
              # relative paths, with "/nix/store/${hash}-source/" stripped.
              srcFilter =
                path: type:
                (craneLib.filterCargoSources path type) || (relSrcFilter (trimStorePathPrefix path) type);
            in
            lib.cleanSourceWith {
              src = ./.;
              filter = srcFilter;
              # Be reproducible, regardless of the directory name
              name = "omniglot-src";
            };

          baseRustBuildArgs = {
            src = cleanedRustSrc;
            strictDeps = true;
          };

          # Build *just* the cargo dependencies (of the entire workspace), so we
          # can reuse all of that work (e.g. via cachix) when running in CI:
          cargoArtifacts = craneLib.buildDepsOnly baseRustBuildArgs;

          # Common arguments shared across all individual targets:
          individualCrateArgs = baseRustBuildArgs // {
            inherit cargoArtifacts;
          };

          # Common arguments shared across all examples:
          exampleCrateArgs = individualCrateArgs // {
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          };

          fileSetForCrate =
            crate: addlFiles:
            lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions (
                [
                  ./Cargo.toml
                  ./Cargo.lock

                  # Files for the base `omniglot` crate, always required.
                  (craneLib.fileset.commonCargoSources ./omniglot)

                  # We have to include one example for Cargo to not complain about
                  # the wildcard in the `Cargo.toml` workspace members for
                  # `examples/*`. We (somewhat arbitrarily) include the `add`
                  # example, as it is small and unlikely to change.
                  (craneLib.fileset.commonCargoSources ./examples/add)

                  (craneLib.fileset.commonCargoSources crate)
                ]
                ++ addlFiles
              );
            };

          omniglot = craneLib.buildPackage (
            individualCrateArgs
            // {
              inherit cargoArtifacts;
              pname = "omniglot";
              cargoExtraArgs = "-p omniglot";
              src = fileSetForCrate ./omniglot [ ];
            }
          );

          omniglot-example-add = craneLib.buildPackage (
            exampleCrateArgs
            // {
              inherit cargoArtifacts;
              pname = "omniglot-example-add";
              cargoExtraArgs = "-p omniglot-example-add";
              src = fileSetForCrate ./examples/add [
                # Include the libadd sources:
                ./examples/add/libadd
              ];
            }
          );
        };

        treefmt =
          (treefmt-nix.lib.evalModule (pkgs.extend (
            self: super: {
              rustfmt = stableRustToolchain;
            }
          )) ./treefmt.nix).config.build;

        flakePackageSetForRustToolchain = rustToolchain: rec {
          default = omniglot;
          inherit (rustPackagesForRustToolchain rustToolchain)
            omniglot
            omniglot-example-add
            ;
        };

      in
      rec {
        packages = flakePackageSetForRustToolchain stableRustToolchain;

        # Check formatting and build all packages for both stable Rust and MSRV:
        checks = {
          formatting = treefmt.check self;
        }
        // (lib.mapAttrs' (n: v: lib.nameValuePair "${n}-stable" v) (
          flakePackageSetForRustToolchain stableRustToolchain
        ))
        // (lib.mapAttrs' (n: v: lib.nameValuePair "${n}-msrv" v) (
          flakePackageSetForRustToolchain msrvRustToolchain
        ));

        formatter = treefmt.wrapper;

        devShells.default = pkgs.mkShell {
          name = "omniglot-devshell";

          packages = with pkgs; [
            stableRustToolchain
          ];

          shellHook = ''
            export LIBCLANG_PATH="${pkgs.libclang.lib}/lib"
          '';
        };
      }
    );
}
