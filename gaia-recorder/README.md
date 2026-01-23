# gaia-recorder

衛星テレメトリとコマンドの記録サービス。REST APIとgRPCインターフェースを提供します。

## アーキテクチャ概要

gaia-recorderは、衛星地上局運用向けに設計されたテレメトリ記録・再生サービスです。テレメトリデータ（TMIV）とコマンド（TCO）をDuckDBデータベースに保存し、gRPCによるデータ取り込みとREST APIによるクエリインターフェースの両方を提供します。

### 主な機能

- **デュアルインターフェース**: リアルタイムテレメトリ取り込み用のgRPC、クエリ用のREST API
- **永続ストレージ**: インデックス最適化されたDuckDBベースのストレージ
- **再生モード**: 記録されたセッションを分析するための読み取り専用モード
- **フィールド名変換**: gRPC形式（`SH_TI@RAW`）とデータベース形式（`SH.TI:raw`）間の自動変換
- **値型の正規化**: レガシー形式をサポートする後方互換性のある型システム
- **時間軸選択**: プライマリ（衛星）タイムスタンプまたは受信（地上局）タイムスタンプでクエリ
- **スマートダウンサンプリング**: min-max-avgアルゴリズムを使用した大規模データセットの自動データ削減

## モジュール構造

コードベースは明確な責任を持つ集中したモジュールに編成されています：

```
gaia-recorder/
├── src/
│   ├── main.rs              (~120行) - アプリケーションエントリポイントと配線
│   ├── lib.rs               - バイナリ向けのパブリックAPIエクスポート
│   ├── api/                 - HTTPおよびgRPC APIレイヤー
│   │   ├── mod.rs           - APIモジュールエクスポート
│   │   ├── grpc.rs          - gRPCサービス実装（RecorderService）
│   │   └── http.rs          - REST APIハンドラーとルーター
│   ├── db/                  - データベース操作
│   │   ├── mod.rs           - データベースモジュールエクスポート
│   │   ├── schema.rs        - スキーマ初期化
│   │   ├── queries.rs       - クエリ操作（テレメトリ、コマンド、時間範囲）
│   │   └── insert.rs        - 挿入操作（テレメトリ、コマンド）
│   ├── domain/              - ドメイン型とビジネスロジック
│   │   ├── mod.rs           - ドメインモジュールエクスポート
│   │   ├── session.rs       - Session管理（SessionInfo、RecordingInfo、SessionState）
│   │   └── telemetry.rs     - ValueType enumと後方互換性
│   ├── transform/           - データ変換ユーティリティ
│   │   ├── mod.rs           - 変換モジュールエクスポート
│   │   └── field_names.rs   - FieldName変換（gRPC ↔ データベース）
│   └── bin/
│       ├── import-csv.rs    - CSVインポートユーティリティ
│       └── list-fields.rs   - フィールド一覧ユーティリティ
├── Cargo.toml
└── README.md
```

## モジュールの責務

### `main.rs` - アプリケーションエントリポイント
- CLI引数パース（`--bind-addr`, `--bind-port`, `--data-dir`, `--playback-mode`）
- ログ初期化
- 状態セットアップ
- gRPCおよびHTTPサービスの配線
- サーバーライフサイクル管理

### `api/` - APIレイヤー
**目的**: HTTPおよびgRPCインターフェースをビジネスロジックから分離

- **`grpc.rs`**: gRPC RecorderトレイトをimplementするRecorderService
  - `post_telemetry`: TMIVパケットの取り込み
  - `post_command`: TCOパケットの取り込み
- **`http.rs`**: REST APIハンドラーとルーター作成
  - セッション管理（`/api/recording/start`, `/api/recording/stop`）
  - テレメトリクエリ（`/api/telemetry/query`）
  - コマンドクエリ（`/api/commands/query`）
  - 時間範囲クエリ（`/api/telemetry/time_range`）
  - 記録一覧（`/api/recordings/list`）
  - スキーマエンドポイント（`/api/schema`）

### `db/` - データベースレイヤー
**目的**: DuckDBによる全てのデータベース操作をカプセル化

- **`schema.rs`**: データベース初期化とテーブル作成
  - 複合インデックス付き`telemetry_samples`テーブル
  - 時間インデックス付き`command_logs`テーブル
- **`queries.rs`**: 読み取り操作
  - `query_telemetry`: ダウンサンプリング付きテレメトリ取得
  - `query_commands`: コマンドログ取得
  - `query_time_range`: min/maxタイムスタンプ取得
  - 内部ダウンサンプリング関数（stride、min-max-avg）
- **`insert.rs`**: 書き込み操作
  - `insert_telemetry_sample`: TMIVフィールドサンプルを保存
  - `insert_command_log`: TCOコマンドログを保存
  - `build_params_json`: コマンドパラメータをシリアライズ

### `domain/` - ドメイン型
**目的**: バリデーション付きビジネスドメイン型を定義

- **`session.rs`**: セッション管理
  - `SessionInfo` struct: 記録セッション情報
  - `RecordingInfo` struct: 記録ファイルメタデータ
  - `Session` struct: 静的メソッド（create、resolve_path、list_all）
  - `SessionState` trait: セッション情報を含む状態の抽象化
- **`telemetry.rs`**: `ValueType` enum
  - 正規形式: `integer`, `double`, `string`, `enum`, `bytes`, `unknown`
  - 後方互換性: レガシー`int`, `num`, `text`形式をサポート
  - 変換メソッド: `to_db_string()`, `from_db_string()`

### `transform/` - データ変換
**目的**: フォーマット変換の一元化

- **`field_names.rs`**: `FieldName` struct
  - gRPC形式: `SH_TI`または`SH_TI@RAW`（アンダースコア、`@RAW`サフィックス）
  - データベース形式: `SH.TI:conv`または`SH.TI:raw`（ドット、`:conv`/`:raw`サフィックス）
  - 包括的なテストカバレッジを持つ双方向変換

## データフロー

### テレメトリ取り込み（gRPC）
```
衛星 → gRPC PostTelemetry
    → RecorderService::post_telemetry
    → transform::FieldName::from_grpc (SH_TI@RAW → SH.TI:raw)
    → db::insert_telemetry_sample
    → DuckDB telemetry_samplesテーブル
```

### テレメトリクエリ（REST API）
```
フロントエンド → GET /api/telemetry/query
             → api::http::query_telemetry
             → resolve_session_path (session_id → db_path)
             → db::query_telemetry
             → 必要に応じてダウンサンプリング（数値型はmin-max-avg、非数値型はstride）
             → value_typeの正規化（domain::ValueType::from_db_string）
             → JSONレスポンス
```

### CSVインポート
```
CSVファイル → import-csvバイナリ
          → transform::FieldName::from_grpc（カラム名）
          → domain::ValueType（int→integer、num→double、text→string）
          → 型推論付きDuckDB COPY FROM
          → telemetry_samplesテーブル
```

## データベーススキーマ

### `telemetry_samples`
```sql
CREATE TABLE telemetry_samples (
    id INTEGER PRIMARY KEY,
    tmiv_name VARCHAR NOT NULL,           -- TMIVパケット名
    field_name VARCHAR NOT NULL,          -- :rawまたは:convサフィックス付きフィールド名
    is_raw TINYINT NOT NULL,              -- 0=変換済み、1=生データ
    time_primary_ms BIGINT NOT NULL,      -- 衛星タイムスタンプ
    time_received_ms BIGINT NOT NULL,     -- 地上局タイムスタンプ
    value_type VARCHAR(20) NOT NULL,      -- integer、double、enum、string、bytes
    value_num DOUBLE,                     -- 数値（integer、double、bytes≤8）
    value_int BIGINT,                     -- 整数値
    value_text VARCHAR,                   -- テキスト値（enumラベル、string）
    value_bytes BLOB                      -- 生バイト
);

CREATE INDEX idx_telemetry_query
    ON telemetry_samples (tmiv_name, field_name, is_raw, time_primary_ms);
```

### `command_logs`
```sql
CREATE TABLE command_logs (
    id INTEGER PRIMARY KEY,
    time_ms BIGINT NOT NULL,              -- コマンドタイムスタンプ
    command_name VARCHAR NOT NULL,        -- TCOコマンド名
    params_json VARCHAR NOT NULL          -- JSONシリアライズされたパラメータ
);

CREATE INDEX idx_command_time ON command_logs (time_ms);
```

## フィールド名形式

### gRPC形式（入力）
- 通常テレメトリ: `SH_TI`, `OBC_MM_OPSMODE`
- 生テレメトリ: `SH_TI@RAW`, `OBC_MM_OPSMODE@RAW`
- 大文字小文字を区別しないサフィックス: `@raw`も受け入れ

### データベース形式（保存）
- 通常テレメトリ: `SH.TI:conv`, `OBC.MM.OPSMODE:conv`
- 生テレメトリ: `SH.TI:raw`, `OBC.MM.OPSMODE:raw`
- アンダースコアをドットに置換
- サフィックスは変換状態を示す

### 変換ルール
1. `_`を`.`に置換: `SH_TI` → `SH.TI`
2. 存在する場合`@RAW`/`@raw`サフィックスを除去
3. 生テレメトリには`:raw`、変換済みテレメトリには`:conv`を追加

## 値型

### 正規形式（新）
- `integer` - 64ビット符号付き整数
- `double` - 64ビット浮動小数点
- `string` - テキスト文字列
- `enum` - テキストラベル付き列挙型
- `bytes` - 生バイト配列
- `unknown` - 未知または欠落した型

### レガシー形式（後方互換）
- `int` → `integer`
- `num` → `double`
- `text` → `string`

### 型保存戦略
- **数値型**（`integer`, `double`）: `value_num`および`value_int`に保存
- **テキスト型**（`enum`, `string`）: `value_text`に保存
- **バイト型**: `value_bytes`に保存、≤8バイトの場合は`value_int`/`value_num`にも保存

## ダウンサンプリングアルゴリズム

クエリが`max_points`より多くのポイントを返す場合、自動ダウンサンプリングが適用されます：

### 数値データ用（Min-Max-Avg）
1. データを`max_points / 3`個のバケットに分割
2. 各バケットでmin、max、avgを計算
3. バケットごとに3ポイント（min、max、avg）をバケットの中央タイムスタンプで出力
4. 値の範囲とトレンドを保持

### 非数値データ用（Stride）
1. ストライドを計算: `samples.len() / max_points`
2. N番目ごとのサンプルを取得
3. 時間的分布を保持

## 動作モード

### 記録モード（デフォルト）
- gRPCテレメトリとコマンドを受け入れ
- `--data-dir`内に`recording_YYYYMMDD_HHMMSS[_suffix].duckdb`を作成
- REST APIで記録セッションの開始/停止が可能
- `session_id`で複数セッションをクエリ可能

#### 自動一時セッション
記録モードで起動すると、gaia-recorderは自動的に一時セッション（`_auto`サフィックス付き）を作成します。この機能により：

- 起動直後からテレメトリをグラフ表示可能
- フロントエンドのPLAYボタンを押すと、セッションが保存されます
- PLAYボタンを押さずに終了した場合、一時データベース（`*_auto.duckdb`）は自動削除されます
- Ctrl+Cによるグレースフルシャットダウンで、一時ファイルのクリーンアップが実行されます

これにより、手動で記録開始する必要なく、即座にデータの可視化が可能になります。

### 再生モード（`--playback-mode`）
- 読み取り専用: `/api/recording/start`と`/api/recording/stop`は403を返す
- 新しいデータベースファイルは作成されない（一時セッションも作成されません）
- 既存の記録をクエリ可能
- 時間範囲クエリは自動的にデータベース境界に調整される
- オフライン分析に有用

## APIエンドポイント

### セッション管理
- `POST /api/recording/start` - 新しい記録セッションを開始（`suffix`が必要）
- `POST /api/recording/stop` - 現在の記録セッションを停止
- `GET /api/recording/session` - 現在のセッション情報を取得

### クエリ
- `GET /api/telemetry/query` - テレメトリサンプルをクエリ
  - パラメータ: `session_id`, `tmiv_name`, `field_name`, `is_raw`, `start_ms`, `end_ms`, `max_points`, `time_axis`
- `GET /api/commands/query` - コマンドログをクエリ
  - パラメータ: `session_id`, `start_ms`, `end_ms`, `max_points`
- `GET /api/telemetry/time_range` - min/maxタイムスタンプを取得
  - パラメータ: `session_id`

### メタデータ
- `GET /api/recordings/list` - 全記録セッションを一覧表示
- `GET /api/schema` - 衛星スキーマJSONを取得（`--schema-file`提供時）

## 設定

### コマンドライン引数
```bash
gaia-recorder \
  --bind-addr 0.0.0.0 \           # バインドアドレス（デフォルト: 0.0.0.0）
  --bind-port 8920 \              # バインドポート（デフォルト: 8920）
  --data-dir ./recordings \       # データディレクトリ（デフォルト: recordings）
  --playback-mode \               # 読み取り専用再生モードを有効化
  --schema-file schema.json       # オプションの衛星スキーマファイル
```

### 環境変数
全てのCLI引数は環境変数で設定可能：
- `BIND_ADDR`, `BIND_PORT`, `DATA_DIR`, `PLAYBACK_MODE`, `SCHEMA_FILE`

## セットアップ

### 前提条件
- Rust (cargo)
- Node.js v21+ & pnpm

### インストール

#### オプション1: npm/pnpm 経由（推奨）
プロジェクトルートで以下を実行すると、gaia-recorder と tmtc-c2a を含むすべてのツールがインストールされます：

```bash
pnpm install
```

これにより、以下のバイナリが `node_modules/.bin/` にインストールされます：
- `gaia-recorder` - テレメトリ記録・再生サービス
- `tmtc-c2a` - 地上局制御ソフトウェア
- `tlmcmddb-cli` - テレメトリ/コマンドデータベースツール
- `kble` / `kble-c2a` - KBLE 関連ツール

インストール後、以下のコマンドで起動できます：

```bash
# レコーダーとtmtc-c2aを同時起動
pnpm run dev

# レコーダーのみ起動
pnpm run run:recorder

# 再生モード
pnpm run playback
```

#### オプション2: Cargo から直接ビルド
```bash
cargo build --release
# バイナリは target/release/gaia-recorder に生成されます
```

## バイナリ

### `gaia-recorder`（メインサービス）
```bash
# レコーダーを起動
cargo run --release

# 再生モードで起動
cargo run --release -- --playback-mode --data-dir /path/to/recordings
```

### `import-csv`（CSVインポートユーティリティ）
```bash
cargo run --release --bin import-csv -- \
  --input-dir /path/to/csv \
  --output-db recording.duckdb \
  --session-id 20250120_190015
```

期待されるCSV構造：
- TMIV CSVファイルを含む`TLM/`ディレクトリ
- TCO CSVファイルを含む`CMD/`ディレクトリ
- gRPC形式のカラム名（`SH_TI`, `SH_TI@RAW`）
- 最初のカラムはタイムスタンプ

### `list-fields`（フィールド一覧ユーティリティ）
記録データベース内の利用可能な全フィールドを一覧表示します。

## テスト

```bash
# 全テストを実行
cargo test

# 特定のモジュールテストを実行
cargo test domain::telemetry
cargo test transform::field_names

# リリースバイナリをビルド
cargo build --release
```

## パフォーマンスに関する考慮事項

### インデックス
- `(tmiv_name, field_name, is_raw, time_primary_ms)`の複合インデックスにより高速クエリが可能
- command_logsの時間インデックスにより効率的な時間的クエリが可能

### DuckDB機能
- 自動圧縮によりディスク使用量を削減
- 最適化されたデータ型（is_rawにTINYINT、タイムスタンプにBIGINT）
- テレメトリバイトの効率的なBLOBストレージ

### ダウンサンプリング
- `samples.len() > max_points`の場合に自動適用
- デフォルト`max_points` = テレメトリ2000、コマンド10000
- Min-max-avgは数値データの値範囲を保持
- Strideサンプリングは時間的分布を維持

## 設計上の決定

### なぜDuckDB？
- **組み込み型**: 別のデータベースサーバーが不要
- **高性能**: 分析クエリに最適化されたカラムナーストレージ
- **ポータブル**: 単一ファイルデータベース、バックアップ/転送が容易
- **圧縮**: 自動圧縮によりストレージコストを削減

### なぜgRPCとRESTを分離？
- **gRPC**: 衛星システムからの低遅延テレメトリ取り込み
- **REST**: フロントエンド/ツール統合のためのシンプルなHTTPクエリ
- **フォールバックサービス**: gRPCはAxumフォールバックで処理、単一ポート

### なぜフィールド名変換？
- **gRPC互換性**: protobufフィールド名では`_`が標準
- **データベース明確性**: 階層的フィールド名には`.`がより読みやすい
- **生データインジケータ**: `:raw`対`:conv`サフィックスでデータ処理状態を明確化
- **フロントエンド一貫性**: 単一形式によりフロントエンドロジックが簡素化

### なぜ値型の正規化？
- **データ一貫性**: 全ての新しいデータは正規型を使用
- **後方互換性**: レガシー型を持つ既存データベースも動作
- **マイグレーション不要**: データベースマイグレーションが不要、透過的パース
- **型安全性**: Enumベースの型はコンパイル時にエラーをキャッチ

## 将来の機能強化

まだ実装されていない潜在的な改善領域：

1. **コネクションプーリング**: 高頻度取り込み用のDuckDBコネクションプール
2. **構造化エラー型**: カスタムエラーenumで`anyhow::Error`を置換
3. **APIバージョニング**: 後方互換性のためのREST APIエンドポイントのバージョニング
4. **メトリクス**: クエリレイテンシ、取り込みレート用のPrometheusメトリクス
5. **圧縮チューニング**: DuckDB圧縮設定の実験
6. **クエリキャッシング**: 頻繁な時間範囲クエリのキャッシュ
7. **バッチ挿入**: 複数TMIVフィールドを単一トランザクションにグループ化

## トラブルシューティング

### バイナリ更新時の「Text file busy」
レコーダープロセスが実行中です。まず停止してください：
```bash
pkill gaia-recorder
cp target/release/gaia-recorder <destination>
```

### 再生モードでクエリ結果が空
session_idが`--data-dir`内の記録と一致するか確認：
```bash
curl http://localhost:8920/api/recordings/list | jq .
```

### フィールド名が見つからない
フィールド名形式に`:raw`または`:conv`サフィックスが含まれているか確認：
- 正しい: `SH.TI:conv`, `SH.TI:raw`
- 誤り: `SH.TI`, `SH_TI@RAW`

gRPC取り込みにはgRPC形式（`SH_TI`, `SH_TI@RAW`）を使用してください。サービスが変換を処理します。

## ライセンス

MPL-2.0

## 参考資料

- [DuckDBドキュメント](https://duckdb.org/docs/)
- [Axum Webフレームワーク](https://docs.rs/axum/)
- [Tonic gRPC](https://docs.rs/tonic/)
