{
  description = "Tools for deploying WebAssembly into Enarx Keeps.";

  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";
  inputs.fenix.url = github:nix-community/fenix;
  inputs.flake-compat.flake = false;
  inputs.flake-compat.url = github:edolstra/flake-compat;
  inputs.flake-utils.url = github:numtide/flake-utils;
  inputs.nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;

  outputs = { self, nixpkgs, fenix, flake-utils, ... }:
    with flake-utils.lib.system; flake-utils.lib.eachSystem [
      aarch64-darwin
      aarch64-linux
      x86_64-darwin
      x86_64-linux
    ]
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          cargo.toml = builtins.fromTOML (builtins.readFile "${self}/Cargo.toml");

          rust.dev = fenix.packages.${system}.fromToolchainFile {
            file = "${self}/rust-toolchain.toml";
          };

          rust.build = with fenix.packages.${system}; combine [
            minimal.cargo
            minimal.rustc
            targets.aarch64-unknown-linux-musl.latest.rust-std
            targets.wasm32-wasi.latest.rust-std # required for tests
            targets.x86_64-unknown-linux-musl.latest.rust-std
            targets.x86_64-unknown-none.latest.rust-std
          ];

          buildPackage = pkgs: extraArgs: (pkgs.makeRustPlatform {
            rustc = rust.build;
            cargo = rust.build;
          }).buildRustPackage
            (extraArgs // {
              inherit (cargo.toml.package) name version;

              src = pkgs.nix-gitignore.gitignoreRecursiveSource [
                "*.nix"
                "*.yml"
                "/.github"
                "/docs"
                "/README-DEBUG.md"
                "/SECURITY.md"
                "deny.toml"
                "flake.lock"
                "LICENSE"
                "rust-toolchain.toml"
              ]
                self;

              cargoLock.lockFileContents = builtins.readFile "${self}/Cargo.lock";

              postPatch = ''
                patchShebangs ./helper
              '';

              buildInputs = pkgs.lib.optional pkgs.stdenv.isDarwin
                pkgs.darwin.apple_sdk.frameworks.Security;
            });

          dynamicBin = buildPackage pkgs {
            cargoTestFlags = [ "wasm::" ];
          };

          stdenvGcc = pkgs: with pkgs.stdenv; "${cc}/bin/${cc.targetPrefix}gcc";

          staticBin = buildPackage pkgs.pkgsStatic
            {
              CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";

              depsBuildBuild = [
                pkgs.stdenv.cc
              ];

              meta.mainProgram = cargo.toml.package.name;
            }
          // pkgs.lib.optionalAttrs (system == aarch64-linux) {
            CARGO_BUILD_TARGET = "aarch64-unknown-linux-musl";
            CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = stdenvGcc pkgs.pkgsMusl;

            # TODO: verify that the binary is indeed static
          }
          // pkgs.lib.optionalAttrs (system == x86_64-linux) {
            CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = stdenvGcc pkgs.pkgsMusl;

            postInstall = ''
              ldd $out/bin/${cargo.toml.package.name} | grep -q 'statically linked' || (echo "binary is not statically linked"; exit 1)
            '';
          };

          staticAarch64Bin = buildPackage pkgs.pkgsStatic
            ({
              CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static";
              CARGO_BUILD_TARGET = "aarch64-unknown-linux-musl";
              CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = stdenvGcc (
                if system == aarch64-linux then pkgs
                else pkgs.pkgsCross.aarch64-multiplatform
              ).pkgsMusl;

              depsBuildBuild = [
                pkgs.stdenv.cc
              ];

              meta.mainProgram = cargo.toml.package.name;
            });

          ociImage = pkgs.dockerTools.buildImage {
            inherit (cargo.toml.package) name;
            tag = cargo.toml.package.version;
            contents = [
              staticBin
            ];
            config.Cmd = [ cargo.toml.package.name ];
            config.Env = [ "PATH=${staticBin}/bin" ];
          };
        in
        {
          defaultPackage = dynamicBin;

          packages = {
            "${cargo.toml.package.name}" = dynamicBin;
            "${cargo.toml.package.name}-static" = staticBin;
            "${cargo.toml.package.name}-static-aarch64" = staticAarch64Bin;
            "${cargo.toml.package.name}-docker" = ociImage;
          };

          devShell = pkgs.mkShell {
            buildInputs = [
              rust.dev
            ];
          };
        }
      );
}
