{
  rustPlatform,
  fontconfig,
  libxkbcommon,
  pkg-config,
}:
rustPlatform.buildRustPackage {
  src = ./.;
  name = "kickoff";
  buildInputs = [fontconfig libxkbcommon];
  nativeBuildInputs = [pkg-config];
  cargoLock.lockFile = ./Cargo.lock;
}
