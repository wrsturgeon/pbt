{ pkgs, ... }:
{
  programs = builtins.mapAttrs (_k: v: { enable = true; } // v) {
    deadnix = { };
    keep-sorted = { };
    mdformat = { };
    nixfmt = {
      package = pkgs.nixfmt-rfc-style;
      strict = true;
    };
    rustfmt = { };
    statix = { };
  };
  projectRootFile = "flake.nix";
  settings.formatter = {
    deadnix.priority = 1;
    statix.priority = 2;
    keep-sorted.priority = 3;
    nixfmt.priority = 4;
  };
}
