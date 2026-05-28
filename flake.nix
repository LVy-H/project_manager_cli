{
  description = "Wardex - Ward & index your workspace: CTF management, project organization, and more";

  # Advertise the project's Cachix cache. With this, `nix run
  # github:LVy-H/wardex` on a cold machine fetches the prebuilt binary
  # instead of compiling the full dep graph from source. Users are prompted
  # to accept this config on first use (standard Nix trust behavior).
  nixConfig = {
    extra-substituters = [ "https://wardex.cachix.org" ];
    extra-trusted-public-keys = [
      "wardex.cachix.org-1:eh2m+FilW3nJgjryJTvANtK/R0pp5N42Eke4ta1l3LE="
    ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;

        # crane uses the toolchain from rust-overlay (same rustc/cargo the
        # devshell uses), ensuring dep artifacts built by Nix are
        # bit-identical to what cargo would produce in `nix develop`.
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Source filter: cargo sources + README.md (src/lib.rs does
        # `include_str!("../README.md")` as the crate-level rustdoc, so
        # build fails if README.md is filtered out).
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (baseNameOf (toString path) == "README.md")
            || (craneLib.filterCargoSources path type);
          name = "source";
        };

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

        buildInputs = with pkgs; [
          openssl
          libgit2
          zlib
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        commonArgs = {
          inherit src nativeBuildInputs buildInputs;
          pname = "wardex";
          version = "0.3.0-alpha3";
          strictDeps = true;
          OPENSSL_NO_VENDOR = 1;
        };

        # Build *only* the dependency graph. This derivation is cached by
        # the hash of Cargo.lock + toolchain; changes to wardex's own
        # source do not invalidate it.
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        wardex = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          # git is needed for integration tests (git init in test fixtures)
          nativeCheckInputs = [ pkgs.git ];
          meta = with pkgs.lib; {
            description = "Ward & index your workspace - CTF management, project organization, and more";
            homepage = "https://github.com/LVy-H/wardex";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "wardex";
          };
        });

      in
      {
        packages.default = wardex;

        # Development shell
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            rustToolchain
            rust-analyzer
            cargo-watch
            cargo-edit
          ]);

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          OPENSSL_NO_VENDOR = 1;
        };

        # Expose the app for `nix run`
        apps.default = flake-utils.lib.mkApp {
          drv = wardex;
        };

        # `nix flake check` — re-runs the real test suite against cached
        # deps, so CI and local checks benefit from the same incremental
        # rebuild as `nix build`.
        checks.wardex = wardex;
      }
    ) // {
      overlays.default = final: prev: {
        wardex = self.packages.${prev.system}.default;
      };
      homeManagerModules.default = import ./nix/hm-module.nix { inherit self; };
    };
}
