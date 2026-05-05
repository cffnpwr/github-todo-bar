{
  description = "GitHub Todo Bar";

  nixConfig = {
    extra-substituters = [
      "https://nix-cache.cffnpwr.dev"
      "https://fenix.cachix.org"
    ];
    extra-trusted-public-keys = [
      "cffnpwr-nixpkgs-extras:dmp2DUGwdqawLCPOsOcRxU/NpCO/qA1jha/8rmoSzvA="
      "fenix.cachix.org-1:ecJhr+RdYEdcVgUkjruiYhjbBloIEGov7bos90cZi0Q="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs-extras = {
      url = "github:cffnpwr/nixpkgs-extras";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      nixpkgs,
      flake-parts,
      fenix,
      nixpkgs-extras,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        { pkgs, system, ... }:
        let
          miseConfig = fromTOML (builtins.readFile ./mise.toml);
          treefmtVersion = miseConfig.tools."aqua:numtide/treefmt";

          toolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-ppIWpYzG2wrU9tJUepvivmn/Rkx/PKML5xg3gQxV4qo="; # x-fenix-sha256
          };
        in
        {
          _module.args.pkgs = import nixpkgs {
            inherit system;
            overlays = [
              nixpkgs-extras.overlays.default
            ];
          };

          formatter = pkgs.treefmt.versions."${treefmtVersion}";

          devShells.default = pkgs.mkShell {
            packages = with pkgs; [
              nixd
              nixfmt
              treefmt.versions."${treefmtVersion}"
              toolchain
            ];

            shellHook = ''
              # Only exec into user shell for interactive sessions
              # Skip for non-interactive commands (like VSCode env detection)
              if [ -t 0 ] && [ -z "$__NIX_SHELL_EXEC" ]; then
                export __NIX_SHELL_EXEC=1

                # Detect user's login shell (works on both macOS and Linux)
                if command -v dscl >/dev/null 2>&1; then
                  # macOS
                  USER_SHELL=$(dscl . -read ~/ UserShell | sed 's/UserShell: //')
                elif command -v getent >/dev/null 2>&1; then
                  # Linux
                  USER_SHELL=$(getent passwd $USER | cut -d: -f7)
                else
                  # Fallback: read /etc/passwd directly
                  USER_SHELL=$(grep "^$USER:" /etc/passwd | cut -d: -f7)
                fi

                exec ''${USER_SHELL:-$SHELL}
              fi
            '';
          };
        };
    };
}
