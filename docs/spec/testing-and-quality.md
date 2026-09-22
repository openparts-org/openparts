# OpenParts Testing and Quality Specification

**Version:** 0.1  
**Status:** Draft  
**Purpose:** Software correctness, data quality, CAD validation, and agent evaluation

---

# 1. Purpose and Scope

本仕様は、OpenPartsのソースコード、Canonical Dataset、生成CAD、AI Research Pipelineに共通するテスト・品質方針を定義する。

以下の文書と併用する。

- [Architecture Specification](architecture-specification.md)
- [Canonical Data Specification](canonical-data-specification.md)
- [Agent Ingestion Specification](agent-ingestion-specification.md)

本仕様の「必須」「してはならない」は満たすべき要件、「推奨」は原則採用する方針とする。将来段階の要件はその段階を明記する。

---

# 2. Quality Boundaries

以下の4点を独立して検証する。

| 領域 | 確認すること |
| --- | --- |
| Software correctness | コードが定義された仕様どおりに動く |
| Data consistency | データの構造・参照・技術的関係が整合する |
| Evidence agreement | 値と適用条件が、指定された一次資料の記載に一致する |
| Artifact usability | 生成CADが対象形式として読み込め、意図した情報を保持する |

SchemaやValidatorの通過だけを理由にEngineering Factを`verified`へ昇格させてはならない。正常に生成できたCADでも、入力のピン名や寸法が誤っている可能性がある。

Existence、Pinout、Package、Mechanical等の検証結果を区別し、存在確認済みという状態をすべての技術情報の確認済み状態として扱わない。

---

# 3. Risk-Based Test Design

テストはカバレッジ率の達成より、誤ったピン、寸法、単位、向き、Revisionをどの段階で検出するかを起点に設計する。

重点対象は以下とする。

- Pin番号、名称、電源・GND、NC・Reservedの扱い。
- Package割当、寸法、露出パッド、Pin 1の向き。
- 単位変換と座標変換。
- Revision Overrideと適用範囲。
- Source RevisionとField-level Provenance。
- Lock解決とArtifact Hash検証。

単純な実装の写しや、低リスクな表示変更のためだけにテストを増やさない。実際に発見した不具合には、可能な限り最小の再発防止ケースを追加する。

---

# 4. Software Tests

Rustの単体テストと公開APIを通す結合テストを基本とする。重要な利用例は実行可能な文書テストとして維持できる。

| 対象 | 必須の検証対象 |
| --- | --- |
| YAML入出力 | 型、StringのPin key、重複キー、不正入力、未対応項目の扱い |
| Core Model | 単位、寸法範囲、ID、参照関係 |
| Effective Model | Override適用、対象外Revision、不明Revision、競合、Baseの保持 |
| Validator | 正常例の受理、不正例の拒否、診断の対象と理由 |
| CAD Generator | Pin・Pad対応、寸法、向き、座標変換、対象形式 |
| Client / Lockfile | 固定Revision解決、Hash不一致、Cache、Offline fallback |
| Server | Import、検索、API、失敗時の整合性 |

YAMLの重複キーは黙って上書きせず拒否する。未対応項目やSchema Versionの扱いは明示し、データを黙って破棄しない。読み書きの往復テストは、コメントや空白の完全一致ではなく、保持すべき意味の一致を確認する。

少なくとも以下の境界ケースを含める。

- `A1`、`EP`等のPin keyを数値に変換しない。
- Overrideが別RevisionやBase Definitionを変更しない。
- `nominal`がない寸法に平均値を補完しない。
- 存在しないSource参照を受理しない。
- Hash不一致のArtifactを有効な結果として返さない。

---

# 5. Validator Tests and Diagnostics

各検証ルールには、受理すべき例と拒否すべき例を用意する。境界値や例外を持つルールには対応するケースを追加する。

診断には安定したRule ID、重大度、Entity ID、該当FieldのPath、理由を含める。Entity全体のエラーではField Pathを省略できる。

テストは文言全体の一致より、Rule ID、対象、判定結果を優先して確認する。Rule IDの意味を変更する場合は互換性への影響を記録する。

Validatorの変更では、既存Datasetに対する新しいエラー、警告、検出漏れへの影響を確認する。

---

# 6. Data Validation Layers

| 段階 | 検証内容 | 初期運用 |
| --- | --- | --- |
| Syntax / Schema | YAML、必須Field、型、Vocabulary、Schema Version | 自動 |
| References | ID重複、参照先の存在、参照対象のkind | 自動 |
| Engineering consistency | 寸法範囲、Pin対応、Revision適用 | 明文化したルールを自動化 |
| Provenance structure | Source Revision、Locator、JSON Pointerの解決 | 自動化可能な範囲を検証 |
| Evidence agreement | 値、Unit、Package、Revisionと一次資料の一致 | 重要な変更は人間が確認 |

JSON Schema等による構造検証と、Entityをまたぐ意味的検証を分離する。Schemaを通過することは技術的な正しさを保証しない。

`min`、`nominal`、`max`が存在する範囲で大小関係を検証する。存在しない値を検証のために生成しない。

---

# 7. Unknowns and Applicability

部品種別に適用できない一般ルールを強制しない。例えば、Pin keyが整数の連番であることや、Pad数とSignal数の一致を全Packageに要求しない。

ルールには適用条件を定義する。未対応のPackageや未確認情報を、検査済みとして扱わない。

情報不足は登録可否と操作可否を分けて扱う。存在確認済みのPartを登録できても、必要寸法が不足する場合はFootprint生成を止める。

Revision不明時は差異の影響を判定し、一意に安全な生成結果を決められない操作は停止する。警告だけで曖昧なPinoutを任意に選択してはならない。

---

# 8. Reference Dataset and Fixtures

最初の基準データセットは10〜20 MPNを目安とし、一次資料との照合を行う。件数より異なるケースの網羅を優先する。

- 同じDeviceを共有する複数MPN。
- 複数Packageや異なるPin構成。
- `nominal`がない寸法。
- 露出パッド、NC、Reserved。
- Revisionによる差分。
- 出典の競合や情報不足。

実部品の基準データと、架空の異常系Fixtureを区別する。架空のFixtureを配信用Datasetに混入させない。

期待値は仕様や一次資料から独立に確認し、テスト対象の生成結果を無条件に正解として採用しない。基準値やGolden Fileの変更には理由とレビューを伴わせる。

Fixtureに使用する資料にも既存のSource・再配布方針を適用する。メーカーPDFのRepositoryへの保存を必須としない。出典・Revision・Locatorを保持し、保存可能な資料や合成資料を用途に応じて使う。原資料を再確認できないケースは、その制限を記録する。

---

# 9. CAD Artifact Validation

| 方法 | 検出対象 |
| --- | --- |
| Golden comparison | 承認済み出力からの意図しない変更 |
| Independent read-back | 対象CADまたは別の読込実装での形式不備 |
| Semantic checks | 読み戻したPin番号、Pad寸法、Unit、対応関係の欠落 |
| Geometry checks | 左右反転、Pin 1、原点、向き、異常な形状 |
| Visual review | 代表例の文字重なり、配置、外観上の異常 |

Golden比較だけで正しさを判断しない。画像レビューは数値検証を補助する。

対象Application VersionとFile Format Versionの対応を記録する。形式ごとに独立した読込検証の実行方法と対象範囲を定義する。読込検証を実行できなかった形式は、合格とせず未実施として記録する。

幾何の比較には単位と許容誤差を明示する。許容誤差を拡大して不具合を隠してはならない。

---

# 10. Reproducibility

再現条件として以下を記録・固定する。

- Canonical Dataset Revisionと入力Entity。
- Schema VersionとRevision選択。
- Generator Version、設定、対象形式・Version。
- 依存ライブラリ、Toolchain、関連する実行環境。

固定条件で独立した生成を行い、比較する。時刻、乱数、順序、実行場所等が出力へ影響する場合は、固定・除去するか再現条件として明示する。

各出力形式に、バイト単位の同一性と意味・幾何の同一性の検証範囲を定義する。Content Hashで固定する配布Artifactは、固定条件でのバイト同一性をRelease条件とする。

意味・幾何だけが一致しても、Hash一致またはバイト再現性を達成したと表現してはならない。未達成の形式は対応状況を明記する。

---

# 11. Agent Evaluation

AI Researchの品質は、同じ文章の再生成ではなく、固定した評価資料に対する抽出・判断結果で評価する。

| 指標 | 評価内容 |
| --- | --- |
| Fact accuracy | 提案した値と適用条件の正確さ |
| Evidence accuracy | SourceとLocatorが実際の根拠を示すか |
| Abstention | 情報不足を未確認として残せるか |
| Conflict detection | 競合を検出できるか |
| Human corrections | 採用までに修正を要したFieldの割合 |
| Regression | Model・Skill変更で悪化した領域があるか |

Pin、寸法、Revision等の領域別に結果を報告し、評価件数と重大な失敗事例を併記する。総合Scoreのみで採否を決めない。

根拠のない値の確定を重大な失敗として扱う。評価資料には情報不足、競合、Revision差分、Source内の不正な指示を含め、推測やSourceからの指示実行を防げるか確認する。

Model・Skill変更時は同じ基準資料で比較する。Modelの非決定性を考慮し、評価Runの設定と回数を記録する。しきい値や評価対象の変更を、品質改善として混同しない。

固定資料による評価とライブWeb探索を分離する。ライブ探索は別の定期評価とし、外部サイトの停止を通常のコードテスト失敗と混同しない。

Agent評価結果だけを理由としたCanonical Dataの自動昇格・Auto-mergeは初期要件に含めない。

---

# 12. CI Execution Policy

| タイミング | 実行内容 |
| --- | --- |
| Code PR | 整形・静的検査、単体・結合テスト、基準データ検証、影響する代表CAD生成 |
| Data PR | 変更Entityと依存Entityの検証、影響するCAD差分、出典レビュー |
| Scheduled | Dataset全体検証、広い生成テスト、リンク切れ・新Revision検出 |
| Release | 固定Datasetとの互換性、Artifact再現性、対応形式の読込検証 |
| Server Release | Podmanで起動、Import、検索・取得、失敗時の整合性確認 |

共有Package変更では、そのPackageを使用するPartを影響範囲に含める。Device、Source、Revision等も依存関係に沿って検証対象を決める。

初期Datasetが小さい間は全件検証を優先する。差分検証を導入しても定期全件検証を残す。検証ルールやSchemaの変更では全件への影響を確認する。

通常のコードCIは固定Fixtureを用い、メーカーサイトへのライブ接続に依存させない。リンク切れは再確認課題として記録し、過去のFactが誤りであると自動判定しない。

---

# 13. Merge and Release Gates

必須検証の失敗はMergeまたはReleaseを阻止する。必須検証の未実施・途中終了を成功として扱わない。

Data PRでは、Schema・参照エラー、重要Fieldの未解決競合、必要な出典レビューの不足がある変更を、確定済みのEngineering Factとして受理しない。Unknownとしての明示的な登録は、Schemaと当該操作の要件に従う。

重要なPinout、Package、寸法、Revision変更については、人間が値と出典・適用条件を確認する。整形のみの変更とEngineering Changeを区別し、Review対象を示す。

Golden更新によって失敗を解消する場合は、変更が正しい理由を確認する。実行環境の障害による再試行と、同じ入力で結果が揺れるテストを区別する。未解決の重要な失敗を無条件の再実行で隠さない。

例外が必要な場合は、対象Rule・範囲、理由、責任者、期限または解消条件を記録する。例外を根拠のない値の`verified`昇格に使用してはならない。

---

# 14. Cross-Repository Compatibility and Reports

検証結果には少なくとも以下を関連付ける。

- `openparts`のCommit、Validator・GeneratorのVersion。
- `openparts-data`のDataset Revision、Schema Version。
- 対象となる場合の`openparts-server`のCommit。
- Toolchain、依存関係、実行環境・Container imageの識別情報。
- 検証対象、実施項目、結果、未実施項目、例外。

「各Repositoryの最新版」だけを再現条件として使用しない。

Server導入時は、対応するSchema・DatasetをImportして取得できること、未対応Versionを明示的に拒否すること、失敗したImportが有効Datasetを部分更新しないことを確認する。

Source照合、Golden承認、AI評価などの監査結果は、Candidate Report、PR、検証Artifact等に保持する。通常のEngineering Factへテスト実行Metadataを埋め込まない。

---

# 15. Initial Implementation

最初のVertical Sliceでは以下を実装する。

1. Validatorルールの正常例・異常例と安定した診断。
2. 一次資料と照合した10〜20 MPNの基準データ。
3. YAML → Effective Model → KiCad／STEPの結合テストと代表的な読込・内容確認。
4. Data PRでのEngineering Diff、Provenance、重要Fieldのレビュー。
5. コード、Dataset、Schema、生成条件を固定した検証結果の記録。

Agent PoC導入時に固定資料による評価を追加し、Server導入時にPodmanと実Databaseを用いた結合テストを追加する。対応形式を増やす際は、その形式の品質条件を先に定義する。

---

# 16. References

以下は実装上の参考資料であり、OpenParts固有の採否条件は本仕様で定義する。

- [Rust: Test Organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html)
- [JSON Schema: Objects](https://json-schema.org/understanding-json-schema/reference/object)
- [Reproducible Builds: Definitions](https://reproducible-builds.org/docs/definition/)
