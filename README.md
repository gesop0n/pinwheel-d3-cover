# pinwheel-d3-cover

> 日本語の詳しい解説はこちら: **[README.ja.md](README.ja.md)**

Smaller dominating certificates and exact verification tools for the 3-task, density-bound $5/6$ domain $\mathcal{D}_3$ in real-period pinwheel scheduling, accompanying:

- *A Simple Computer-Assisted Proof of the 5/6-Bound for Real-Period Pinwheel Scheduling* (arXiv:2611.10624)
- Upstream repository: <https://github.com/grzegorz-gutowski/pinwheel_scheduling>

## Main Upper-Bound Results ($\mathcal{D}_3$)

For dimension $n = 3$ and density bound $5/6$,
$$\mathcal{D}_3 = \{(d_1, d_2, d_3) : d_1 \ge d_2 \ge d_3 > 0,\ d_1 + d_2 + d_3 \le 5/6\},$$
the upstream paper certifies domination using **18 words** (maximum length 12). This repository shows that **12 words** suffice at the same maximum length 12, and **11 words** suffice overall (maximum length 24):

| Certificate | Words | Max word length | Notes |
| --- | ---: | ---: | --- |
| Upstream `proof_5_6/D3.txt` | 18 | 12 | Paper's original certificate |
| [`proofs/D3_12words.txt`](proofs/D3_12words.txt) | **12** | 12 | Exact minimum among all words of length $\le 23$ |
| [`proofs/D3_11words.txt`](proofs/D3_11words.txt) | **11** | 24 | Exact minimum among all words of length $\le 24$ (and globally optimal, $N(3) = 11$) |

Every certificate uses the exact same `WORD,b1,b2,b3` format as the upstream repository, passes the upstream paper's official prover directly, passes this repository's solver-free exact Fourier–Motzkin check, and has no redundant words.

## Verifying Directly with the Upstream Paper's Prover

Because `proofs/D3_12words.txt` and `proofs/D3_11words.txt` are plain text certificates, their correctness does **not** depend on how they were discovered (HiGHS is only used to search for minimal covers). You can verify them directly with the upstream paper's official Z3-based prover (`--recompute-boxes` ignores stored box annotations and recomputes every dominance box from the word):

```sh
nix develop .#paper-prover
git clone https://github.com/grzegorz-gutowski/pinwheel_scheduling.git /tmp/pinwheel_scheduling
cd /tmp/pinwheel_scheduling
uv sync && uv run cpp-build

uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/proofs/D3_12words.txt \
  --recompute-boxes --max-steps 0

uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/proofs/D3_11words.txt \
  --recompute-boxes --max-steps 0
```

Expected output for both certificates:

```text
The supplied words dominate the specified domain: dimension 3; density at most 5/6; each density in (0, 1]
```

## Quick Start (Standalone Rust Toolchain)

The Rust implementation is completely self-contained (no dependency on the upstream repository).

```sh
nix develop   # provides rustc, cargo, cmake, pkg-config, LIBCLANG_PATH, elan

# Run all unit tests and certificate verification tests
cargo test --release

# Compute the dominance box B(w) of a word (matches upstream packing_schedule_quality)
cargo run --release -- box AABACB

# Independently verify certificates (solver-free exact rational Fourier–Motzkin + leave-one-out)
cargo run --release -- upper verify proofs/D3_12words.txt
cargo run --release -- upper verify proofs/D3_11words.txt

# Rediscover the minimum 12-word cover from all words of length <= 12
cargo run --release -- upper solve --max-len 12

# Rediscover an 11-word cover from length <= 24 in a few seconds (sum-pruned enumeration)
cargo run --release -- upper solve --max-len 24 --min-sum 19/20
```

## Repository Layout & Correspondence with Upstream

```text
proofs/
  D3_12words.txt         12-word certificate (length <= 12)
  D3_11words.txt         11-word certificate (length <= 24)
src/
  model/
    bound.rs             Fraction, DensityVector, UpperBound (cf. upstream bound.hpp)
    word.rs              DominanceBox, PeriodicSchedule, dominance_box, repair_word
                         (cf. upstream schedule.hpp, plan_pack.hpp, prover.py)
    fm.rs                Exact rational Fourier–Motzkin elimination + witness extraction
  upper/
    enumerate.rs         Parallel FKM Lyndon-word enumeration & sum-pruned DFS
    cells.rs             Maximal-box filtering & top-slice Delta cell arrangement
    mincover.rs          Two-stage minimum set cover via HiGHS (good_lp)
    verify.rs            Solver-free full-domain domination check + leave-one-out
tests/
  upper_certificates.rs  End-to-end verification of proofs/D3_12words.txt and D3_11words.txt
```
