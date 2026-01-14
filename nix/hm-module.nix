{ self }:
{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.wardex;
  yamlFormat = pkgs.formats.yaml { };
in
{
  options.programs.wardex = {
    enable = mkEnableOption "wardex";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = literalExpression "flake.packages.\${pkgs.system}.default";
      description = "The wardex package to install.";
    };

    settings = mkOption {
      type = yamlFormat.type;
      default = { };
      example = literalExpression ''
        {
          paths = {
            workspace = "/home/user/workspace";
          };
        }
      '';
      description = ''
        Configuration written to {file}`$XDG_CONFIG_HOME/wardex/config.yaml`.
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."wardex/config.yaml".source = yamlFormat.generate "wardex-config.yaml" cfg.settings;
  };
}
