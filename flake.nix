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
          default = pkgs.mkShell {
            packages = [
              # Rust ツールチェイン
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.rust-analyzer

              # `highs` クレート（`highs-sys`: CMake + bindgen）のビルドに必要なツール
              # ※ `llvmPackages.libclang` を packages に入れると unwrapped な clang++ が PATH に
              #    混ざり stdenv のラッパを覆い隠すため、下の LIBCLANG_PATH のみで参照する
              pkgs.cmake
              pkgs.pkg-config
            ];

            # bindgen が libclang を見つけられるようにパスを指定
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            shellHook = ''
              echo "pinwheel-d3-cover dev shell: $(rustc --version), $(cargo --version), cmake $(cmake --version | head -1 | awk '{print $3}')"
            '';
          };

          # 以下のソースコードを動かすための環境
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
