{
  description = "MikroTik config generator";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }: let
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
  in {
    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = [ pkgs.bash pkgs.shellcheck pkgs.dotenv-cli ];
      };
    });

    apps = forAllSystems (pkgs: let
      importWireguard = pkgs.writeShellApplication {
        name = "import-wireguard";
        runtimeInputs = [ pkgs.coreutils pkgs.gnugrep pkgs.gnused ];
        text = builtins.readFile ./wg-to-env.sh;
      };

      generateRsc = pkgs.writeShellApplication {
        name = "generate-rsc";
        runtimeInputs = [ ];
        text = builtins.readFile ./generate-rsc.sh;
      };

      generate = pkgs.writeShellApplication {
        name = "generate";
        runtimeInputs = [ pkgs.dotenv-cli generateRsc ];
        text = ''
          dotenv -- generate-rsc "$@"
        '';
      };
    in {
      import-wireguard = { type = "app"; program = "${importWireguard}/bin/import-wireguard"; };
      generate         = { type = "app"; program = "${generate}/bin/generate"; };
    });
  };
}
