{
  description = "Wardex - Ward & index your workspace: CTF management, project organization, and more";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        
        rustToolchain = pkgs.rust-bin.stable.latest.default;
        
        # Native build inputs needed for git2 and other dependencies
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];
        
        # Build inputs (libraries)
        buildInputs = with pkgs; [
          openssl
          libgit2
          zlib
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];
        
      in {
        packages = {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "wardex";
            version = "0.1.0";
            
            src = ./.;
            
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            
            inherit nativeBuildInputs buildInputs;
            
            # Environment variables for building
            OPENSSL_NO_VENDOR = 1;
            
            meta = with pkgs.lib; {
              description = "Ward & index your workspace - CTF management, project organization, and more";
              homepage = "https://github.com/LVy-H/wardex";
              license = licenses.mit;
              maintainers = [];
              mainProgram = "wardex";
            };
          };
        };
        
        # Development shell
        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            rust-analyzer
            cargo-watch
            cargo-edit
          ]);
          
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          OPENSSL_NO_VENDOR = 1;
        };
        
        # Expose the app for `nix run`
        apps.default = flake-utils.lib.mkApp {
          drv = self.packages.${system}.default;
        };
      }
    );
}
