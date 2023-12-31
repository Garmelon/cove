{
  description = "TUI client for euphoria.leet.nu, a threaded real-time chat platform";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, naersk }:
    let forAllSystems = nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed;
    in {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naersk' = pkgs.callPackage naersk { };
          cargoToml = pkgs.lib.importTOML ./Cargo.toml;
        in
        {
          default = naersk'.buildPackage {
            name = "cove";
            version = cargoToml.workspace.package.version;
            root = ./.;
          };
        }
      );
    };
}
