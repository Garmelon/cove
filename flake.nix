{
  description = "TUI client for euphoria.io, a threaded real-time chat platform";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }: flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import nixpkgs { inherit system; };
      naersk' = pkgs.callPackage naersk { };
    in
    rec {
      packages.default = naersk'.buildPackage { src = ./.; };
    }
  );
}
