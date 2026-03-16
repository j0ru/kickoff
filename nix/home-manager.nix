{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.programs.kickoff;
  tomlFormat = pkgs.formats.toml {};
in {
  options.programs.kickoff = {
    enable = lib.mkEnableOption "kickoff launcher";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.kickoff;
      defaultText = lib.literalExpression "pkgs.kickoff";
      description = "The kickoff package to install.";
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = {};
      description = ''
        Configuration written to kickoff's config.toml.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [cfg.package];

    xdg.configFile."kickoff/config.toml".source =
      tomlFormat.generate "kickoff-config.toml" cfg.settings;
  };
}
