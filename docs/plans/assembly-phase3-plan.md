# MusubiCAD Assembly (Phase 3) 実装計画

> Status: Complete (M3.1–M3.3 implemented)
> Scope: `modules/assembly` を中心とした組立モデリング機能
> Related: [Architecture overview](../architecture/overview.md), [AGENTS.md](../../AGENTS.md)

現状 `modules/assembly` は空スタブ（`component.rs` / `instance.rs` / `mate.rs` / `connector.rs` /
`solve.rs` が各1行）。ただし合流点は既に整備済み:

- `modules/core/src/manifest.rs` が `graph/assemblies.json` スロットを予約済み。
- `modules/core/src/id.rs` に `ComponentId`（`component:` プレフィックス）を定義済み。

つまり Assembly は「設計時に想定済みだが未実装」の状態。

---

## 0. 前提と設計判断（着手前に確定すべき点）

| 論点 | 現状 | 推奨 |
|---|---|---|
| ドキュメントモデル | `OcadDocument` は part 専用（sketches/feature_nodes 中心）。assembly は自前形状を持たず子 part を参照する | **(B)** `OcadDocument` に `assembly: Option<AssemblyModel>` を追加し、`metadata.kind = part \| assembly` で区別。既存の CLI/シリアライズ配管を再利用。(A) 別ドキュメント型より変更が小さい |
| ベクトル表現 | 数学ライブラリ無し。`[f64;3]` 配列（`SketchPlacement` は origin/x_axis/y_axis 方式） | 既存慣習に合わせ `[f64;3]` + 3×3 回転で `RigidTransform` を定義（nalgebra 等は ADR 無しでは導入しない） |
| 剛体変換のカーネル | `translate_body` のみ存在。回転付き配置が無い | `GeometryKernel` trait に `transform_body(body, RigidTransform)` を追加 |

**AGENTS.md 準拠**: アーキテクチャ変更のため **ADR-003「Assembly document model」** の追加が必須。
1機能=1PR、テスト必須、`schemas/` とマイグレーション更新必須。

MVP は **静的アセンブリ（配置のみ・拘束ソルブ無し）** に絞る。mate ソルバは次サブマイルストーンに
分離すると、各PRが小さく決定的になる。

---

## 1. データモデル（`modules/assembly`）

```
AssemblyModel
├─ components: Vec<Component>     // 参照する子 part 定義
├─ instances:  Vec<Instance>     // 子 part の配置インスタンス
└─ mates:      Vec<Mate>         // M3.2 で有効化（MVPでは空）

Component  { id: ComponentId, source_path: String(相対), source_doc: DocumentId }
Instance   { id: InstanceId, component: ComponentId, placement: Placement, fixed: bool, name: String }
Placement  { transform: RigidTransform }   // geometry の RigidTransform を保持
```

- `InstanceId`（`instance:` プレフィックス）を `core/src/id.rs` の `define_id!` に追加。
- 全構造体 `Serialize/Deserialize`、`BTreeMap`/ソートで **決定的順序**（AGENTS.md 不変条件）。
- 単位は明示（`_m` サフィックス、`SketchPlacement` の `origin_m` 慣習に合わせる）。

---

## 2. ジオメトリ層（`modules/geometry`）

- `RigidTransform { translation_m: [f64;3], rotation: [[f64;3];3] }` を追加。合成・逆・単位・点写像。
- `GeometryKernel` trait に `fn transform_body(&self, body, xf: RigidTransform) -> Result<KernelBody>` を追加。
- `MockGeometryKernel` に実装（テスト用）。
- **テスト**: 合成の結合則、単位変換=恒等、逆変換で往復、点写像がトレランス一致。

## 3. OCCT 実装（`modules/kernel-occt`）

- `transform_body` を `gp_Trsf` + `BRepBuilderAPI_Transform` で実装（C++ ブリッジ経由）。
- **統合テスト**（OCCT 必須なので integration 指定）: 立方体を回転+並進 → バウンディングボックスが期待通り。

---

## 4. ファイル形式（`modules/file`）

- `OcadDocument` に `#[serde(default, skip_serializing_if="Option::is_none")] pub assembly: Option<AssemblyModel>` を追加（**後方互換**: 既存 part は無変化）。
- `expanded_dir.rs`: `graph/assemblies.json`（マニフェスト予約済みスロット）の write/read を実装。
- `migrate.rs`: assembly 無し旧ドキュメントの読み込み互換。
- `schemas/`: assembly スキーマ追加。
- **テスト**: assemblies.json 往復、part-only ドキュメントの互換読込、canonical JSON 決定性。

## 5. 再生成パイプライン（アセンブリ regen）

配置のみ（拘束ソルブ無し）:

1. 各 `Component.source_path` の子 `.ocad.d` を解決・**既存 part regen パイプラインで再生成** → `KernelBody` を取得。
2. 各 `Instance.placement` を `transform_body` で適用。
3. 全インスタンスを **compound** として集約 → アセンブリシーン。
4. 失敗した子の regen はアセンブリ全体を壊さない（AGENTS.md「失敗した再生成は文書を破壊しない」）→ インスタンス単位のエラー報告。

- **循環参照検出**（component が自身を参照）を検証。
- **テスト（regression/golden）**: bodies 数・総バウンディングボックス・質量が golden 一致。

---

## 6. CLI / 例 / ドキュメント

- `opencad new <path> assembly`: bracket ×2 を配置したテンプレート（`modules/desktop/src/template.rs` に追加。CLI `new` はこれを再利用）。
- `opencad regen`: assembly を検出して子を再生成・配置、`instances: N` を報告。
- `opencad export`: compound を STL 出力。
- `examples/assembly_two_brackets.ocad.d` を追加（AGENTS.md「全機能に例1つ」）。
- `docs/architecture/assembly.md`、`docs/adr/ADR-003-*.md`。

---

## 7. タスク分解（PR単位・順序付き）

### M3.1 — 静的アセンブリ（MVP・このマイルストーンの詳細対象）

| ID | 内容 | モジュール | 主テスト |
|---|---|---|---|
| A01 | ADR-003 + `AssemblyModel`/`Component`/`Instance`/`Placement` + `InstanceId` | core, assembly | 純データ往復 |
| A02 | `RigidTransform` + `transform_body` trait + Mock 実装 | geometry | 変換数学 |
| A03 | OCCT `transform_body` 実装 | kernel-occt | 統合(回転+並進) |
| A04 | `OcadDocument.assembly` + `assemblies.json` 入出力 + マイグレーション | file | 往復/互換 |
| A05 | アセンブリ regen（子解決→配置→compound）+ 循環検出 | assembly/feature | golden regression |
| A06 | CLI `new assembly`/regen/export + 例 + docs | cli, desktop | CLIスモーク |

### M3.2 — Mate 拘束とソルバ（`solver::gauss_newton_solve` を再利用）

- A07: Mate 型（Coincident / Concentric / Distance / Angle / Parallel / Ground）+ `(InstanceId, TopoRef)` 参照 + 検証
- A08: アセンブリ DOF モデル（インスタンスあたり6DOF、grounding で拘束除去）
- A09: Mate 残差 + `gauss_newton_solve` 接続 + DOF/過拘束診断
- A10: CLI/例（拘束付きアセンブリ）

### M3.3 — 統合

- A11: Connector（名前付き座標フレーム＝再利用可能な mate 基準）
- A12: Agent API（assembly の query/patch/diff、DesignPatch 経由）
- A13: render/desktop のマルチインスタンス表示（インスタンス別トランスフォーム＋色）
- A14: サブアセンブリ（ネスト）、アセンブリパターン

---

## 8. 完了条件（Definition of Done）

- `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` すべて通過。
- `examples/assembly_two_brackets.ocad.d` が regen → STL export 可能。
- part-only の既存 `.ocad` が無変更で読める（後方互換）。
- ADR-003 と `docs/architecture/assembly.md` が最新。

---

## 9. 次アクション

- Phase 3 完了。Definition of Done は §8 を参照。
- 次フェーズ候補: **Phase 4 Drawing**（`opencad-drawing`、Task-174+）または CI / 例の E2E 強化。
