{
  description = "pinwheel-d3-cover 用の開発環境";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells = {
          # メイン環境
          default = pkgs.mkShell {
            packages = [
              # Rust ツールチェイン
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer

              # `highs` クレート（`highs-sys`: CMake + bindgen）のビルドに必要なツール
              pkgs.cmake
              pkgs.pkg-config
              pkgs.llvmPackages.libclang

              # Lean 4 のツールチェイン管理（`lean/` 用）
              pkgs.elan
            ];

            # bindgen が libclang を見つけられるようにパスを指定
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            shellHook = ''
              echo "pinwheel-d3-cover dev shell: $(rustc --version), $(cargo --version), cmake $(cmake --version | head -1 | awk '{print $3}'), elan $(elan --version | awk '{print $2}')"
            '';
          };

          # 元論文の公¡式検証器を動かすための任意環境
          #
          # 対象論文:
          #   "A Simple Computer-Assisted Proof of the 5/6-Bound for Real-Period Pinwheel Scheduling"
          #   (arXiv:2611.10624)
          # Github リポジトリ:
          #   https://github.com/grzegorz-gutowski/pinwheel_scheduling
          paper-prover = pkgs.mkShell {
            packages = [
              pkgs.cmake
              pkgs.boost
              pkgs.uv
              pkgs.python313
            ];

            # uv が自動で Python をダウンロードせず、Nix 提供の Python を使うように指定
            UV_PYTHON_PREFERENCE = "only-system";

            shellHook = ''
              echo "paper-prover shell (arXiv:2611.10624): cmake $(cmake --version | head -1 | awk '{print $3}'), boost ${pkgs.boost.version}, $(python3 --version), uv $(uv --version | awk '{print $2}')"
            '';
          };
        };
      }
    );
}
