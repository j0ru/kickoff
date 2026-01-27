{
  description = "Kickoff, a minimalistic program launcher";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      pkgs = nixpkgs.legacyPackages."x86_64-linux";
    in
    {
      devShells."x86_64-linux".default = pkgs.mkShell {
        buildInput = with pkgs; [
          cargo
          rustc
          rustfmt
          clippy
          rust-analyzer
          fontconfig
          libxkbcommon
        ];
        env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
      };
      packages.x86_64-linux.default = pkgs.callPackage ./default.nix { };
    };
}
