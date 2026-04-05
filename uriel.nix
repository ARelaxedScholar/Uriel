{ config, lib, pkgs, ... }:

let
  cfg = config.services.uriel;
in {
  options.services.uriel = {
    enable = lib.mkEnableOption "Uriel Discord-to-Obsidian Bot";

    vaultPath = lib.mkOption {
      type = lib.types.str;
      description = "Absolute path to the Obsidian vault.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.path;
      description = "Path to the environment file containing secrets (e.g. from SOPS).";
    };

    package = lib.mkOption {
      type = lib.types.package;
      description = "The compiled Uriel package.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.uriel = {
      description = "Uriel Discord-to-Obsidian Bot";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      # Ensure external RAG dependencies are available to the bot
      path = with pkgs; [
        ripgrep
        obsidian
      ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/uriel";
        Restart = "always";
        RestartSec = "10s";

        # Load secrets safely
        EnvironmentFile = cfg.environmentFile;
        Environment = "VAULT_PATH=${cfg.vaultPath}";
      };
    };
  };
}
