{
  description = "Dev shell for verifying the pinwheel_scheduling computer-assisted proof (arXiv:2611.10624)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cmake      # upstream/cpp_src needs CMake >= 3.16
            pkgs.boost      # Boost.Program_options (+ BoostConfig.cmake)
            pkgs.uv         # Python deps: pysmt + z3-solver via `uv sync`
            pkgs.python313  # interpreter uv will use (see UV_PYTHON_PREFERENCE)
            pkgs.elan       # Lean 4 toolchain manager (lean/ holds the formalisation)
          ];

          # Make uv use the nix-provided interpreter instead of downloading one.
          UV_PYTHON_PREFERENCE = "only-system";

          shellHook = ''
            echo "pinwheel dev shell: cmake $(cmake --version | head -1 | awk '{print $3}'), boost ${pkgs.boost.version}, $(python3 --version), uv $(uv --version | awk '{print $2}')"
            echo "upstream build:  (cd upstream && uv sync && uv run cpp-build)"
            echo "upstream checks: (cd upstream && uv run check-proofs-5-6)"
          '';
        };
      });
    };
}
