{
  description = "openapi-dart-parser";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs =
    {
      nixpkgs,
      crane,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      systems = lib.systems.flakeExposed;
      forAllSystems =
        with nixpkgs.lib;
        fn:
        genAttrs systems (
          system:
          fn rec {
            inherit system;
            pkgs = import nixpkgs { inherit system; };
            craneLib = crane.mkLib pkgs;
            src = ./.;
            commonArgs = {
              inherit src;
              strictDeps = true;
              nativeBuildInputs = [ pkgs.pkg-config ];
              buildInputs = [ pkgs.openssl ];
            };
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          }
        );
    in
    {
      checks = forAllSystems (
        {
          craneLib,
          cargoArtifacts,
          commonArgs,
          ...
        }:
        {
          openapi_parser = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              doCheck = false;
            }
          );

          openapi_parser-fmt = craneLib.cargoFmt { inherit (commonArgs) src; };

          openapi_parser-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets";
            }
          );

          openapi_parser-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          openapi_parser-nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            }
          );
        }
      );

      apps = forAllSystems (
        {
          craneLib,
          cargoArtifacts,
          commonArgs,
          ...
        }:
        {
          default = {
            type = "app";
            program =
              let
                package = craneLib.buildPackage (
                  commonArgs
                  // {
                    inherit cargoArtifacts;
                  }
                );
              in
              "${package}/bin/openapi_parser";
          };
        }
      );

      devShells = forAllSystems (
        {
          pkgs,
          craneLib,
          ...
        }:
        {
          default = craneLib.devShell (
            {
              packages = [
                pkgs.pkg-config
                pkgs.openssl
              ];
            }
            // lib.optionalAttrs pkgs.stdenv.isDarwin {
              # CodeLLDB's bundled liblldb does not search PATH for `debugserver`
              # on macOS; it consults LLDB_DEBUGSERVER_PATH (or a fixed path
              # inside its own LLDB.framework, which doesn't ship debugserver).
              # Point it at the copy that comes with the Xcode Command Line Tools
              # so debugging Rust tests/binaries from Zed works out of the box.
              LLDB_DEBUGSERVER_PATH = "/Library/Developer/CommandLineTools/Library/PrivateFrameworks/LLDB.framework/Versions/A/Resources/debugserver";
            }
          );
        }
      );
    };
}
