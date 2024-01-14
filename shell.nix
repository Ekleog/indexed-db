let
  pkgs = import ./nix;
in
pkgs.stdenv.mkDerivation {
  name = "indexed-db-rs";
  buildInputs = (
    (with pkgs; [
      cargo-bolero
      cargo-nextest
      chromedriver
      geckodriver
      just
      niv
      wasm-bindgen-cli
      wasm-pack

      (fenix.combine (with fenix; [
        minimal.cargo
        minimal.rustc
        rust-analyzer
        targets.wasm32-unknown-unknown.latest.rust-std
      ]))
    ])
  );
}
