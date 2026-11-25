# pinwheel-d3-cover（日本語解説）

> English README: **[README.md](README.md)**

実周期 Pinwheel Scheduling における 5/6 境界の計算機援用証明論文：

- *A Simple Computer-Assisted Proof of the 5/6-Bound for Real-Period Pinwheel Scheduling* (arXiv:2611.10624)
- 元論文の公式リポジトリ: <https://github.com/grzegorz-gutowski/pinwheel_scheduling>

のうち、**仕事数 $n = 3$・密度和 $5/6$ 以下の領域 $\mathcal{D}_3$** に絞って、領域を覆うのに必要な周期スケジュール（語）の個数をどこまで減らせるかを調べ、探索から独立検証までを Rust で再現できるようにしたリポジトリです。

---

## このリポジトリで示していること

3 つの仕事それぞれの密度を $d_1 \ge d_2 \ge d_3 > 0$ としたとき、対象となる領域は

$$\mathcal{D}_3 = \{(d_1, d_2, d_3) \mid d_1 \ge d_2 \ge d_3 > 0,\ d_1 + d_2 + d_3 \le 5/6\}$$

です。元論文ではこの領域 $\mathcal{D}_3$ を **18 個の語**（最大長 12）で覆っていました。
本リポジトリでは、同じ領域 $\mathcal{D}_3$ をより少ない語数で完全に覆う **証明データ**（元論文では *certificate* と呼ばれる語のリストファイル）を与えています。

| 証明データ | 語数 | 最長の語 | 位置づけ |
| --- | ---: | ---: | --- |
| 元論文 `proof_5_6/D3.txt` | 18 | 12 | 論文に収録されているデータ |
| [`certs/D3_12words.txt`](certs/D3_12words.txt) | **12** | 12 | 長さ 12 以下（実は長さ 23 以下すべて）の中での厳密な最小語数 |
| [`certs/D3_11words.txt`](certs/D3_11words.txt) | **11** | 24 | 長さ 24 以下での最小語数（全語長にわたっても最小、$N(3) = 11$） |

いずれのデータも「1 語でも外すと覆い残しが生じる」極小な集合であり、元論文とまったく同じ書式（`語,上界1,上界2,上界3`）で書かれています。

---

## まず結果だけを確かめる（元論文の公式検証器でチェック）

上界の証明データ（`certs/D3_12words.txt`, `certs/D3_11words.txt`）の正しさは、**それをどうやって見つけたかには一切依存しません。**
元論文の公式リポジトリ付属の検証器（Z3 ベース）にそのまま渡すだけで、第三者が独立に正しさを確認できます。

```sh
# 元論文検証器用の任意 Nix シェルに入る
nix develop .#paper-prover

# 元論文リポジトリを取得してビルド
git clone https://github.com/grzegorz-gutowski/pinwheel_scheduling.git /tmp/pinwheel_scheduling
cd /tmp/pinwheel_scheduling
uv sync && uv run cpp-build

# 本リポジトリの 12 語・11 語の証明データをそのまま検証
# （--recompute-boxes により、ファイル内の箱の注釈は信用せず語から再計算する）
uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/certs/D3_12words.txt \
  --recompute-boxes --max-steps 0

uv run python pinwheel_scheduling_packing_prover.py \
  --dimension 3 --density-bound 5/6 \
  --certificate /path/to/pinwheel-d3-cover/certs/D3_11words.txt \
  --recompute-boxes --max-steps 0
```

どちらも次のメッセージが出て終了コード 0 になれば、$\mathcal{D}_3$ 全体が完全に覆われていることが元論文の検証器そのもので確認できます。

```text
The supplied words dominate the specified domain: dimension 3; density at most 5/6; each density in (0, 1]
```

---

## どうやって最小の語数を見つけたか（仕組みの概要）

元論文のツールは「覆われていない点を見つけたら、その点を覆う語を 1 つ足す」という貪欲な手続きで 18 語を構成しているため、語数の最小化は行いません。
本リポジトリでは、次の 4 段階で最小の語の組を直接求めています。

1. **支配箱 $B(w)$ の計算**（`src/model/word.rs`）
   長さ $n$ の語 $w$ を無限に繰り返すスケジュールが各仕事に許す密度の上限を、有効性の定義から厳密な有理数（`<` と `<=` の区別つき）で計算します。
2. **候補語の列挙**（`src/upper/enumerate.rs`）
   回転で一致する語や短い語の繰り返しは同じスケジュールになるため、FKM アルゴリズムで**原始ネックレス（Lyndon 語）**だけを並列列挙し、異なる支配箱ごとに最短の代表語を集めます。また、上端の和 $u_1 + u_2 + u_3 \ge \sigma$ で枝刈りする高速版（`--min-sum`）を使えば、長さ 24 以下でも数秒で極大な箱をすべて集められます。
3. **連続領域を有限の集合被覆問題に落として最小化**（`src/upper/cells.rs`, `src/upper/mincover.rs`）
   - 支配箱は下に閉じているため、$\mathcal{D}_3$ 全体を覆うことは上側の切り口 $\Delta = \mathcal{D}_3 \cap \{d_1 + d_2 + d_3 = 5/6\}$ を覆うことと同値です。
   - 候補箱の閾値で各座標軸を点と開区間に切ると、直積で作られる各細胞の中では「どの箱に入るか」が一定になります。
   - $\Delta$ と交わる細胞を厳密有理数の **Fourier–Motzkin 消去**で洗い出し、有限の集合被覆問題として混合整数計画ソルバ **HiGHS**（`good_lp`）で最小語数（同数なら総語長最小）を解きます。
4. **ソルバを使わない独立検証**（`src/upper/verify.rs`）
   見つかった証明データについて、今度はスライスへの帰着も外部ソルバ（HiGHS / Z3）も一切使わず、$\mathcal{D}_3$ 全体に対して厳密有理数の Fourier–Motzkin 消去だけで「覆い残しが 0 であること」と「どの 1 語を外しても覆い残しの有理点が出ること」を検算します。

> **なぜ探索に HiGHS、検証に Z3 / Fourier–Motzkin を使うのか？**
> HiGHS は「238 個の候補から選ぶ個数を最小にする」ような組合せ最適化が得意な一方、内部計算は浮動小数点です。逆に元論文の Z3 や本リポジトリの Fourier–Motzkin 消去は任意精度の有理数で `<` と `<=` を誤差なく判定できます。探索と検証で道具を分けることで、**高速に見つけて 100% 厳密に確かめる**構成にしています。

---

## このリポジトリ単体での動かし方

元論文リポジトリをクローンしなくても、Rust だけで探索から独立検証まで完結します。

```sh
# 開発環境に入る（Rust, cmake, pkg-config, LIBCLANG_PATH, elan を提供）
nix develop

# 単体テスト＋証明データ（12語・11語）の自動検証をまとめて実行
cargo test --release

# 1語の支配箱を計算（元論文の packing_schedule_quality と同じ書式で出力）
cargo run --release -- box AABACB
# -> AABACB,<1/2,<=1/3,<=1/6

# 証明データをソルバ不要の Fourier–Motzkin 消去で独立検証（各語を外した反例点も表示）
cargo run --release -- upper verify certs/D3_12words.txt
cargo run --release -- upper verify certs/D3_11words.txt

# 長さ 12 以下の全語から最小 12 語を再発見する（約 0.1 秒）
cargo run --release -- upper solve --max-len 12

# 長さ 24 以下（枝刈り u1+u2+u3 >= 19/20）から 11 語をゼロから再発見する（数秒）
cargo run --release -- upper solve --max-len 24 --min-sum 19/20
```

---

## 元論文コードとの対応

ソースコード内の型名・用語・コメントは、元論文リポジトリ（英語）と照らし合わせて読めるように揃えています。

| 概念 | 元論文リポジトリ | 本リポジトリ（Rust） |
| --- | --- | --- |
| 厳密有理数 | `Fraction` | `model::bound::Fraction`（`Ratio<i128>`） |
| 密度の上界（`<` / `<=`） | `UpperBound`（`bound.hpp`, `prover.py`） | `model::bound::UpperBound` |
| 語と支配箱の組 | `PeriodicSchedule` / `PlanPack` | `model::word::PeriodicSchedule` |
| 語から支配箱を計算 | `Schedule::get_upper_bound`（`schedule.hpp`） | `model::word::task_upper_bound` / `dominance_box` |
| ラベル降順化＋最小回転 | `PlanPack::repair`（`plan_pack.hpp`） | `model::word::repair_word` |
| 覆い残し点の探索 | `UndominatedVectorFinder`（Z3） | `upper::verify::find_undominated_density_vector`（FM 消去） |
