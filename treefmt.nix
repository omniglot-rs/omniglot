{ pkgs, ... }:
{
  projectRootFile = "flake.nix";

  programs.nixfmt = {
    enable = true;
  };

  programs.rustfmt = {
    enable = true;
    edition = "2024";
  };
}
