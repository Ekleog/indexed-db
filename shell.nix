let
  pkgs = import ./nix;
in
pkgs.stdenv.mkDerivation {
  name = "indexed-db-rs";
  buildInputs = (
    (with pkgs; [
      cargo-bolero
      cargo-nextest
      #chromedriver
      #chromium
      firefox
      geckodriver
      just
      niv
      wasm-bindgen-cli
      wasm-pack

      (fenix.combine (with fenix; [
        minimal.cargo
        minimal.rustc
        complete.rust-src
        rust-analyzer
        targets.wasm32-unknown-unknown.latest.rust-std
      ]))
    ])
  );
}
