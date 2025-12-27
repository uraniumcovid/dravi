{
  description = "DraVi - A mathematical drawing tool with Typst export";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "dravi";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
          ];
          
          buildInputs = with pkgs; [
            glibc
          ];

          meta = with pkgs.lib; {
            description = "A TUI mathematical drawing tool with Typst export";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo
            rust-analyzer
            typst
            pkg-config
            glibc
            # Terminal PDF viewers
            zathura
          ];
        };
      }
    );
}