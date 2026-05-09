{
  description = "MikroTik config generator";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }: let
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
  in {
    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = [ pkgs.cargo pkgs.rustc ];
      };
    });

    apps = forAllSystems (pkgs: let
      package = pkgs.rustPlatform.buildRustPackage {
        pname = "mikrotik-travel-router";
        version = "0.1.0";
        src = self;
        cargoLock.lockFile = ./Cargo.lock;
      };
    in {
      import-wireguard = { type = "app"; program = "${package}/bin/import-wireguard"; };
      generate         = { type = "app"; program = "${package}/bin/generate"; };
    });
  };
}
