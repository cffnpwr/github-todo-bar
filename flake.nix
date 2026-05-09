{
  description = "GitHub Todo Bar";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        { pkgs, ... }:
        {
          devShells.default = pkgs.mkShell {
            # Rust toolchain (rustup), treefmt, yamlfmt, cargo-llvm-cov 等は mise が管理する。
            # ここでは mise では提供できない system C ライブラリのみ供給する。
            # libiconv: libc クレートが macOS で `#[link(name = "iconv")]` を宣言しており、
            #           libc に依存する全 Rust バイナリのリンクに必要。
            packages = [
              pkgs.libiconv
            ];
          };
        };
    };
}
