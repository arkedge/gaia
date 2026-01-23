# GAIA Recorder Scripts

gaia-recorderの起動と運用を補助するスクリプト集。

## start-recorder.sh

gaia-recorderをRTモード（またはプレイバックモード）で起動し、終了時にGドライブへ自動バックアップするスクリプト。

### 機能

- gaia-recorderをRTモードで起動（オプションでプレイバックモードも可能）
- 終了時にrecordingsディレクトリ内の全DBファイルをGドライブに自動バックアップ
- robocopyによる差分コピー（既存ファイルはスキップ）
- ファイル検証とエラーハンドリング

### 使用方法

```bash
# gaia-recorderディレクトリから実行
cd gaia-recorder
./scripts/start-recorder.sh [options]
```

### オプション

- `--playback-mode`: RTモードの代わりにプレイバックモードで起動
- `--no-backup`: 終了時のGドライブバックアップをスキップ

### 例

```bash
# RTモードで起動（通常使用）
./scripts/start-recorder.sh

# プレイバックモードで起動
./scripts/start-recorder.sh --playback-mode

# バックアップなしで起動
./scripts/start-recorder.sh --no-backup

# 終了時は Ctrl+C を押す
# → 自動的にサービスが停止
# → recordingsディレクトリ内の全DBファイルがGドライブにバックアップ
```

### 必要なファイル

- `example/tlmcmddb.json` - テレメトリ・コマンド定義
- `boom-tools/bin/gaia-recorder` - gaia-recorderバイナリ（pnpm installで自動インストール）

### データベースの自動バックアップ

終了時（Ctrl+C）に自動的にデータベースファイルをGドライブにバックアップします。

- バックアップ元: `recordings/*.duckdb`
- バックアップ先: `/mnt/g/共有ドライブ/ArkEdge Users/HirokiHarada/zatsu/ログデータ/gaia-recorder`
- robocopyを使用して差分コピー（既に存在するファイルはスキップ）
- バックアップに失敗してもスクリプトは正常終了します

### ログファイル

gaia-recorderのログは以下に出力されます：

- `/tmp/gaia-recorder.log`

エラーが発生した場合は、このログファイルを確認してください。

---

## start-viewer.sh

録画データ（CSV zip形式）をgaia-recorderで閲覧するための起動スクリプト。

### 機能

- ZIPファイルからCSVデータをDuckDBデータベースに自動変換
- gaia-recorderをプレイバックモードで起動（組み込みフロントエンドを使用）
- 終了時に一時データベースファイルを自動削除
- 必要なバイナリの自動ビルド

### 使用方法

```bash
# gaia-recorderディレクトリから実行
cd gaia-recorder
./scripts/start-viewer.sh [options] <zip_file>
```

### オプション

- `--reuse-db`: 既存のデータベースがあれば確認なしで再利用（CSVインポートをスキップ、終了時も保持）

### 例

```bash
# 通常使用: ZIPファイルから録画データを閲覧
./scripts/start-viewer.sh /path/to/recording.zip

# 既存DBを再利用（CSVインポートをスキップして高速起動）
./scripts/start-viewer.sh --reuse-db /path/to/recording.zip

# ブラウザでdevtoolsを開く
# http://localhost:8920/devtools/

# 終了時は Ctrl+C を押す
# → 自動的にサービスが停止
# → オプションなしの場合は一時DBファイルも削除される
```

### 必要なファイル

- `example/tlmcmddb.json` - テレメトリ・コマンド定義
- `boom-tools/bin/gaia-recorder` - gaia-recorderバイナリ（pnpm installで自動インストール）
- `target/release/import-csv` - CSV→DuckDB変換ツール（初回実行時に自動ビルド）

### データベースの命名規則

インポートされたデータベースファイルは、ZIPファイル名をそのまま使用して命名されます：

```
ZIPファイル: /path/to/my_recording.zip
→ データベース: recordings/my_recording.duckdb
```

これにより、元のファイルとの紐づけが明確になり、複数の録画データを扱う際も識別が容易になります。

### データベースの再利用

オプションなしで起動し、既に変換済みのデータベースが存在する場合、再利用するか確認されます。

```
Database already exists: recordings/my_recording.duckdb
Reuse existing database? (y/N) y
```

- `y`: 既存のDBを再利用（変換をスキップ、終了時も削除しない）
- `N`: 既存のDBを削除して再変換

`--reuse-db`オプションを使用すると、この確認をスキップして自動的に既存DBを再利用します。

### ログファイル

gaia-recorderのログは以下に出力されます：

- `/tmp/gaia-recorder.log`

エラーが発生した場合は、このログファイルを確認してください。

### 注意事項

1. **データベースサイズ**: CSVデータの約20倍のサイズになります
   - 例: 70MBのCSV → 約1.6GBのDuckDB
   - 十分なディスク容量を確保してください

2. **インポート時間**: データ量に応じて数分かかる場合があります
   - 約17万レコード/分の処理速度
   - `--release`ビルドにより高速化

3. **必要なファイル**:
   - `example/tlmcmddb.json`

### トラブルシューティング

#### gaia-recorderが起動しない

```bash
cat /tmp/gaia-recorder.log
```

#### データベースサイズが大きすぎる

手動で削除：
```bash
rm -f recordings/recording_*.duckdb
```

#### ポートが既に使用されている

既存のプロセスを終了：
```bash
# gaia-recorder (port 8920)
lsof -ti:8920 | xargs kill -9
```
