{
  description = "Kickoff, a minimalistic program launcher";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs @ {
    flake-parts,
    home-manager,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        home-manager.flakeModules.home-manager
      ];
      systems = ["x86_64-linux" "aarch64-linux"];

      perSystem = {
        pkgs,
        system,
        ...
      }: {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          name = "kickoff";
          buildInputs = [pkgs.fontconfig pkgs.libxkbcommon];
          nativeBuildInputs = [pkgs.pkg-config];
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [pkgs.rustPlatform.buildRustPackage];
          buildInputs = [pkgs.fontconfig pkgs.libxkbcommon];
          nativeBuildInputs = [pkgs.pkg-config pkgs.rustc pkgs.cargo pkgs.rust-analyzer];
        };
      };

      flake = {
        homeModules.default = import ./nix/home-manager.nix;
      };
    };
}
