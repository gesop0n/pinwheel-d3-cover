# pinwheel-d3-cover

> 日本語の解説: **[README.ja.md](README.ja.md)**

Smaller dominating certificates and verification tools for the 3-task, density-bound $5/6$ domain $\mathcal{D}_3$ in real-period pinwheel scheduling, accompanying:

- *A Simple Computer-Assisted Proof of the 5/6-Bound for Real-Period Pinwheel Scheduling* (arXiv:2611.10624)
- Paper's repository: <https://github.com/grzegorz-gutowski/pinwheel_scheduling>

## Upper-Bound Results ($\mathcal{D}_3$)

For $n = 3$ tasks with ordered densities $d_1 \ge d_2 \ge d_3 > 0$ and density bound $5/6$,

$$\mathcal{D}_3 = \{(d_1, d_2, d_3) : d_1 \ge d_2 \ge d_3 > 0,\ d_1 + d_2 + d_3 \le 5/6\}.$$

| Certificate | Words | Max word length | Notes |
| --- | ---: | ---: | --- |
| Paper's `packing_plans/proof_5_6/D3.txt` | 18 | 12 | Paper's certificate (Table 1) |
| [`proofs/D3_12words.txt`](proofs/D3_12words.txt) | **12** | 12 | Minimum word count among lengths $\le 23$ (found by HiGHS; this 12-word certificate comes from `--max-len 12`) |
| [`proofs/D3_11words.txt`](proofs/D3_11words.txt) | **11** | 24 | Minimum word count among lengths $\le 24$ (found by HiGHS) |

Scope of guarantees:

- **Strictly verified (domination and leave-one-out necessity):** Both certificates in `proofs/` dominate all of $\mathcal{D}_3$, and removing any single word leaves an undominated point. This is checked both by the paper's Z3 prover and by this repository's solver-free exact rational Fourier–Motzkin checker.
- **Solver-computed (minimum word count):** The claims that no smaller cover exists for lengths $\le 23$ (12 words) and $\le 24$ (11 words) rely on exhaustive enumeration and HiGHS (floating-point MIP); optimality certificates are not exported. No claim is made here for word lengths $\ge 25$.

## Verifying with the Paper's Prover

Both certificates use the same `WORD,b1,b2,b3` format as the paper's repository, so their domination of $\mathcal{D}_3$ can be checked independently of how they were found using the paper's Z3-based prover (`--recompute-boxes` ignores stored box annotations and recomputes every dominance box from the word):

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

Expected output for both certificates (exit code 0):

```text
The supplied words dominate the specified domain: dimension 3; density at most 5/6; each density in (0, 1]
```

## How It Works

The paper's prover builds a certificate iteratively by adding a word whenever an undominated point is found, without aiming to minimize the number of words. This repository searches for minimum-size covers in three stages and verifies the result separately:

1. **Candidate enumeration (`src/upper/enumerate.rs`, `src/model/word.rs`)**
   - Computes each word's dominance box $B(w)$ in exact rational arithmetic (`<` vs `<=` preserved) from the validity condition (paper eq. (2)).
   - Exhaustive mode enumerates primitive necklaces (Lyndon words) in parallel via FKM and keeps one shortest canonical representative per distinct density-ordered box.
   - Sum-pruned DFS (`--min-sum 19/20`) prunes prefixes whose upper bound on $u_1 + u_2 + u_3$ falls below $\sigma$. This is a heuristic; empirically, for all tested lengths up to 24 it yields the same inclusion-maximal boxes as exhaustive enumeration (233 maximal boxes in $\mathcal{D}_3$ at length 24).
2. **Finite set-cover reduction on the top slice (`src/upper/cells.rs`)**
   - Caps each box at $\mathcal{D}_3$'s coordinate suprema and keeps only inclusion-maximal boxes.
   - Because dominance boxes are down-closed, dominating $\mathcal{D}_3$ is equivalent to dominating the 2D slice $\Delta = \mathcal{D}_3 \cap \{d_1 + d_2 + d_3 = 5/6\}$.
   - Splits each axis into points and open intervals at all box thresholds and uses exact rational Fourier–Motzkin elimination (`Ratio<i128>`, overflow checks enabled in release builds) to extract feasible cells on $\Delta$ and reduce them to inclusion-minimal set-cover constraints.
3. **Minimum cover via HiGHS (`src/upper/mincover.rs`)**
   - Stage 1 minimizes the word count $k$ over the finite set-cover constraints using HiGHS (`good_lp`).
   - Stage 2 minimizes total word length at cardinality $k$ **within the inclusion-maximal box pool** (not globally over all words: shorter non-maximal words such as `ABC` drop out of the pool once length-14 words enter, so `--max-len 23` returns 12 words of total length 120 including length-14 words, whereas `--max-len 12` returns 12 words of total length 114).
4. **Independent full-domain verification (`src/upper/verify.rs`)**
   - `upper solve` and `upper verify` check the resulting certificate directly over the full 3D domain $\mathcal{D}_3$ (without assuming the slice reduction or calling HiGHS/Z3) using `Ratio<i128>` Fourier–Motzkin elimination (with `overflow-checks = true` in release builds), confirming 0 undominated points and producing an explicit rational counterexample for each leave-one-out removal.

## Quick Start

This crate does not depend on the paper's repository (it builds the C++ HiGHS solver via CMake and `libclang`, provided by `nix develop`).

```sh
nix develop   # provides rustc, cargo, cmake, pkg-config, LIBCLANG_PATH

# Run unit tests and certificate verification tests
cargo test --release

# Compute the canonicalized dominance box B(w) of a 3-task word
# (matches the paper's packing_schedule_quality for words using all of {A, B, C})
cargo run --release -- box AABACB

# Independently verify certificates (solver-free exact FM over full D_3 + leave-one-out)
cargo run --release -- upper verify proofs/D3_12words.txt
cargo run --release -- upper verify proofs/D3_11words.txt

# Find a minimum 12-word cover among words of length <= 12 (~0.1 s)
cargo run --release -- upper solve --max-len 12

# Find an 11-word cover among length <= 24 via sum-pruned DFS (~10-20 s on 10 cores)
cargo run --release -- upper solve --max-len 24 --min-sum 19/20

# Exhaustive enumeration without sum-pruning (reproduces minimum word counts for <= 23 and <= 24)
cargo run --release -- upper solve --max-len 23   # ~5 min on 10 cores -> 12 words
cargo run --release -- upper solve --max-len 24   # ~15 min on 10 cores -> 11 words
```

Minimum covers are not unique, so `upper solve` may return a different valid cover with the same word count (for example, another 11-word cover at length $\le 24$).

## Repository Layout & Correspondence with the Paper's Code

```text
proofs/
  D3_12words.txt         12-word certificate (max length 12)
  D3_11words.txt         11-word certificate (max length 24)
src/
  model/
    bound.rs             Fraction (Ratio<i128>), DensityVector, UpperBound (cf. bound.hpp)
    word.rs              DominanceBox, PeriodicSchedule, dominance_box, repair_word
                         (cf. schedule.hpp, plan_pack.hpp, pinwheel_scheduling_packing_prover.py)
    fm.rs                Exact rational Fourier–Motzkin elimination + witness extraction
  upper/
    enumerate.rs         Parallel FKM Lyndon-word enumeration & sum-pruned DFS
    cells.rs             Maximal-box filtering & top-slice Delta cell arrangement
    mincover.rs          Two-stage minimum set cover via HiGHS (good_lp)
    verify.rs            Solver-free full-domain domination check + leave-one-out
tests/
  upper_certificates.rs  Verification of proofs/D3_12words.txt, D3_11words.txt, and ignored L<=24 test
```
