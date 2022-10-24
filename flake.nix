{
  description = "A tool to interactively write shell pipelines.";

  inputs = {
    utils.url = github:numtide/flake-utils;
    nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;

    naersk = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = github:nix-community/naersk;
    };

    fenix = {
      inputs.nixpkgs.follows = "nixpkgs";
      url = github:nix-community/fenix;
    };
  };

  outputs = { self, utils, nixpkgs, naersk, fenix }:
    utils.lib.eachDefaultSystem (
      system: let
        pname = "pipr";
        pkgs = import nixpkgs { inherit system; overlays = [ fenix.overlay ]; };

        fenix-packages = fenix.packages.${system};

        pkg = let
          naersk-lib = naersk.lib.${system}.override {
            inherit (fenix-packages.minimal) cargo rustc;
          };
        in naersk-lib.buildPackage { inherit pname; root = ./.; };

        target-static = "x86_64-unknown-linux-musl";
        pkg-static = let
          toolchain = with fenix.packages.${system}; combine [
            minimal.rustc
            minimal.cargo
            targets.${target-static}.latest.rust-std
          ];

          naersk-lib = naersk.lib.${system}.override {
            cargo = toolchain;
            rustc = toolchain;
          };
        in naersk-lib.buildPackage {
          inherit pname;
          root = ./.;
          nativeBuildInputs = with pkgs; [
            pkgsStatic.stdenv.cc
          ];

          CARGO_BUILD_TARGET = target-static;
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-statc";
        };

        docker-image = pkgs.dockerTools.buildLayeredImage {
          name = pkg-static.pname;
          tag = pkg-static.version;
          contents = [ pkg-static ];
          config.Cmd = [ "/bin/${pname}" ];
        };
      in
        rec {
          packages = with pkgs; {
            inherit docker-image;
            ${pname} = pkg;
            "${pname}-static" = pkg-static;
          };

          defaultPackage = packages.${pname};

          apps.${pname} = utils.lib.mkApp { drv = packages.${pname}; };
          defaultApp = packages.${pname};

          devShell = pkgs.mkShell {
            nativeBuildInputs = let
              rust = with fenix-packages; combine [
                minimal.cargo
                minimal.rustc
                latest.rust-src
                targets.${target-static}.latest.rust-std
              ];
            in with pkgs; [
              rust rust-analyzer-nightly cargo-watch
              bubblewrap
            ];
          };
        }
    );
}

