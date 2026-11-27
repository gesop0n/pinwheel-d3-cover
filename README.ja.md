# pinwheel-d3-cover（日本語解説）

> English README: **[README.md](README.md)**

実周期 Pinwheel Scheduling における 5/6 境界の計算機援用証明論文：

- *A Simple Computer-Assisted Proof of the 5/6-Bound for Real-Period Pinwheel Scheduling* (arXiv:2611.10624)
- 元論文のリポジトリ: <https://github.com/grzegorz-gutowski/pinwheel_scheduling>

のうち、**仕事数 $n = 3$・密度和 $5/6$ 以下の領域 $\mathcal{D}_3$** に絞って、領域を覆う周期スケジュール（語）の個数をどこまで減らせるかを探索し、独立に検証するリポジトリです。

---

## このリポジトリで示していること

3 つの仕事それぞれの密度を $d_1 \ge d_2 \ge d_3 > 0$ としたとき、対象領域は

$$\mathcal{D}_3 = \{(d_1, d_2, d_3) \mid d_1 \ge d_2 \ge d_3 > 0,\ d_1 + d_2 + d_3 \le 5/6\}$$

です。本リポジトリでは、この領域 $\mathcal{D}_3$ を覆う **証明データ**（元論文のリポジトリでの *certificate*、書式 `語,上界1,上界2,上界3`）を与えています。

| 証明データ | 語数 | 最長の語 | 位置づけ |
| --- | ---: | ---: | --- |
| 元論文 `packing_plans/proof_5_6/D3.txt` | 18 | 12 | 論文に収録されているデータ（Table 1） |
| [`proofs/D3_12words.txt`](proofs/D3_12words.txt) | **12** | 12 | 長さ 23 以下の全語における最小語数（HiGHS による計算結果。同梱データは `--max-len 12` から取得） |
| [`proofs/D3_11words.txt`](proofs/D3_11words.txt) | **11** | 24 | 長さ 24 以下の全語における最小語数（HiGHS による計算結果） |

保証の範囲は次のとおりです。

- **厳密に検証済み（被覆と極小性）**：同梱の 12 語・11 語はいずれも $\mathcal{D}_3$ 全体を覆い、1 語でも外すと覆い残しが生じます。これは元論文の Z3 検証器と、本リポジトリのソルバを使わない Fourier–Motzkin 消去の両方で確認できます。
- **ソルバによる計算結果（語数の最小性）**：「長さ 23 以下では 11 語以下にできない」「長さ 24 以下では 10 語以下にできない」という最小性の主張は、全列挙と浮動小数点 MIP ソルバ HiGHS の最適性判定に依存しており、最適性の証明は出力されません。また、長さ 25 以上の語を含めた最小語数については本リポジトリでは主張していません。

---

## 結果だけを確かめる（元論文の検証器でチェック）

`proofs/D3_12words.txt` と `proofs/D3_11words.txt` は元論文のリポジトリと同じ書式のテキストファイルであり、被覆の正しさは見つけ方に依存しません。元論文のリポジトリ付属の Z3 ベース検証器にそのまま渡して確認できます。

```sh
# 元論文の検証器用 Nix シェルに入る
nix develop .#paper-prover

# 元論文のリポジトリを取得してビルド
git clone https://github.com/grzegorz-gutowski/pinwheel_scheduling.git /tmp/pinwheel_scheduling
cd /tmp/pinwheel_scheduling
uv sync && uv run cpp-build

# 本リポジトリの 12 語・11 語の証明データを検証
# （--recompute-boxes により、ファイル内の箱の注釈は使わず語から再計算する）
uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/proofs/D3_12words.txt \
  --recompute-boxes --max-steps 0

uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/proofs/D3_11words.txt \
  --recompute-boxes --max-steps 0
```

どちらも次のメッセージが出て終了コード 0 になれば、$\mathcal{D}_3$ 全体が覆われていることが確認できます。

```text
The supplied words dominate the specified domain: dimension 3; density at most 5/6; each density in (0, 1]
```

---

## 探索と検証の仕組み

元論文のツールは「覆われていない点が返るたびに、その点を覆う語を追加する」という手順で構成しており、語数の最小化は目的としていません。本リポジトリでは、次の 3 段階で最小語数の組を求め、最後に独立に検証しています。

1. **支配箱 $B(w)$ の計算と候補語の列挙**（`src/model/word.rs`, `src/upper/enumerate.rs`）
   - 長さ $L$ の語 $w$ を繰り返すスケジュールが各仕事に許す密度の上限を、有効性の定義（論文式 (2)）から厳密な有理数（`<` と `<=` の区別つき）で計算します。
   - 全列挙モードでは、回転や短い語の繰り返しによる重複を避けるため FKM アルゴリズムで**原始ネックレス（Lyndon 語）**だけを並列列挙し、異なる支配箱ごとに最短の正準代表語を集めます。
   - 枝刈り DFS（`--min-sum 19/20`）では、上端の和 $u_1 + u_2 + u_3 \ge \sigma$ を満たさない枝を途中で刈ります。これは高速化のためのヒューリスティックであり、和が $19/20$ 未満の極大な箱が存在しない証明はありませんが、長さ 24 以下のテストした全語長で全列挙と同じ極大な箱の集合（長さ 24 で $\mathcal{D}_3$ 上 233 個）が得られることを確認しています。
2. **連続領域を有限の集合被覆問題に落とす**（`src/upper/cells.rs`）
   - 各座標を $\mathcal{D}_3$ の上限で丸め、包含極大な箱だけを候補プールに残します。
   - 支配箱は下に閉じているため、$\mathcal{D}_3$ 全体を覆うことは上側の平面スライス $\Delta = \mathcal{D}_3 \cap \{d_1 + d_2 + d_3 = 5/6\}$ を覆うことと同値です。
   - 候補箱の閾値で各座標軸を点と開区間に刻み、$\Delta$ と交わる直積細胞を 128 ビット整数の有理数（`Ratio<i128>`）による **Fourier–Motzkin 消去**で洗い出して、包含極小な制約行に圧縮します。
3. **HiGHS による最小集合被覆の求解**（`src/upper/mincover.rs`）
   - 有限の集合被覆問題として混合整数計画ソルバ **HiGHS**（`good_lp`）で最小語数 $k$ を求め、第 2 段階で**極大な箱のプール内において**総語長最小の解を選びます。
   - ※第 2 段階は極大な箱のプール内に限られるため、全語にわたる総語長最小ではありません（たとえば長さ 14 以上を許すと `ABC` は長い語の箱に包含されてプールから外れるため、`--max-len 23` の出力は長さ 14 の語を含む総語長 120 の 12 語になり、最大長 12・総語長 114 の同梱 `proofs/D3_12words.txt` とは一致しません）。
4. **独立検証**（`src/upper/verify.rs`）
   - `upper solve` の出力前および `upper verify` では、平面スライスへの帰着も外部ソルバ（HiGHS / Z3）も使わず、元の 3 次元領域 $\mathcal{D}_3$ 全体に対して `Ratio<i128>` の Fourier–Motzkin 消去（リリースビルドでも `overflow-checks = true` でオーバーフローを検出）だけで「覆い残しが 0 であること」と「どの 1 語を外しても具体的な覆い残しの有理点が出ること」を検算します。

> **探索と検証で道具を分ける理由**
> HiGHS は数百候補からの組合せ最適化が高速な一方、内部計算は浮動小数点です。逆に元論文の Z3（任意精度）や本リポジトリの Fourier–Motzkin 消去（128 ビット整数有理数＋オーバーフロー検査）は `<` と `<=` を誤差なく判定できます。探索と検証を分けることで、高速に候補を探しつつ被覆は厳密に確かめる構成にしています。

---

## このリポジトリ単体での動かし方

元論文のリポジトリをクローンせずに探索から検証まで実行できます（内部で C++ の HiGHS を CMake と `libclang` 経由でビルドします）。

```sh
# 開発環境に入る（rustc, cargo, cmake, pkg-config, LIBCLANG_PATH を提供）
nix develop

# 単体テスト＋証明データ（12語・11語）の検証をまとめて実行
cargo test --release

# 3 仕事の語の支配箱を計算（正準化して出力。3 文字すべて使う語では元論文の packing_schedule_quality と一致）
cargo run --release -- box AABACB
# -> AABACB,<1/2,<=1/3,<=1/6

# 証明データをソルバ不要の Fourier–Motzkin 消去で独立検証（各語を外した反例点も表示）
cargo run --release -- upper verify proofs/D3_12words.txt
cargo run --release -- upper verify proofs/D3_11words.txt

# 長さ 12 以下の全語から最小 12 語の組を探索（約 0.1 秒）
cargo run --release -- upper solve --max-len 12

# 長さ 24 以下から枝刈り DFS（u1+u2+u3 >= 19/20）で 11 語の組を探索（10 コアで約 10〜20 秒）
cargo run --release -- upper solve --max-len 24 --min-sum 19/20

# 枝刈りなしの全列挙（長さ 23 以下・24 以下での最小語数の再現）
cargo run --release -- upper solve --max-len 23   # 10 コアで約 5 分 → 12 語
cargo run --release -- upper solve --max-len 24   # 10 コアで約 15 分 → 11 語
```

最小語数の被覆は一意ではないため、`upper solve` は同梱の証明データと同じ語数の別解を返すことがあります。

---

## 元論文のコードとの対応

ソースコード内の型名・用語・コメントは、元論文のリポジトリ（英語）と照らし合わせて読めるように揃えています。

| 概念 | 元論文のリポジトリ | 本リポジトリ（Rust） |
| --- | --- | --- |
| 厳密有理数 | `Fraction`（任意精度） | `model::bound::Fraction`（`Ratio<i128>`） |
| 密度の上界（`<` / `<=`） | `UpperBound`（`bound.hpp`, `pinwheel_scheduling_packing_prover.py`） | `model::bound::UpperBound` |
| 語と支配箱の組 | `PeriodicSchedule` / `PlanPack` | `model::word::PeriodicSchedule` |
| 語から支配箱を計算 | `Schedule::get_upper_bound`（`schedule.hpp`） | `model::word::task_upper_bound` / `dominance_box` |
| ラベル降順化＋最小回転 | `PlanPack::repair`（`plan_pack.hpp`） | `model::word::repair_word` |
| 覆い残し点の探索 | `UndominatedVectorFinder`（Z3） | `upper::verify::find_undominated_density_vector`（FM 消去） |
