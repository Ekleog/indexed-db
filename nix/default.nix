
let
  sources = import ./sources.nix;
in
import sources.nixpkgs {
  overlays = [
    (import (sources.fenix + "/overlay.nix"))
  ];
}
