# GAIA Scripts

## start-recorder.sh

gaia-recorderをRTモード（またはプレイバックモード）で起動し、終了時にGドライブへ自動バックアップするスクリプト。

### 機能

- gaia-recorderをRTモードで起動（オプションでプレイバックモードも可能）
- 終了時にrecordingsディレクトリ内の全DBファイルをGドライブに自動バックアップ
- rsyncによる差分コピー（既存ファイルはスキップ）

### 使用方法

```bash
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

### データベースの自動バックアップ

終了時（Ctrl+C）に自動的にデータベースファイルをGドライブにバックアップします。

- バックアップ元: `gaia-recorder/recordings/*.duckdb`
- バックアップ先: `/mnt/g/共有ドライブ/ArkEdge Users/HirokiHarada/zatsu/ログデータ`
- rsyncを使用して差分コピー（既に存在するファイルはスキップ）
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
- gaia-recorderをプレイバックモードで起動
- tmtc-c2aを起動してdevtoolsを提供
- 終了時に一時データベースファイルを自動削除

### 使用方法

```bash
./scripts/start-viewer.sh [options] <zip_file>
```

### オプション

- `--reuse-db`: 既存のデータベースがあれば確認なしで再利用（CSVインポートをスキップ、終了時も保持）

### 例

```bash
# 通常使用: ZIPファイルから録画データを閲覧
./scripts/start-viewer.sh tmtc-c2a/260113-0822-comet-ae-rp-staging.zip

# 既存DBを再利用（CSVインポートをスキップして高速起動）
./scripts/start-viewer.sh --reuse-db tmtc-c2a/260113-0822-comet-ae-rp-staging.zip

# ブラウザでdevtoolsを開く
# http://localhost:8900/devtools/
# または http://localhost:XXXX/devtools/

# 終了時は Ctrl+C を押す
# → 自動的にすべてのサービスが停止
# → オプションなしの場合は一時DBファイルも削除される
```

### データベースの再利用

オプションなしで起動し、既に変換済みのデータベースが存在する場合、再利用するか確認されます。

```
Database already exists: gaia-recorder/recordings/recording_20260113_0823.duckdb
Reuse existing database? (y/N) y
```

- `y`: 既存のDBを再利用（変換をスキップ、終了時も削除しない）
- `N`: 既存のDBを削除して再変換

`--reuse-db`オプションを使用すると、この確認をスキップして自動的に既存DBを再利用します。

### ログファイル

各サービスのログは以下に出力されます：

- gaia-recorder: `/tmp/gaia-recorder.log`
- tmtc-c2a: `/tmp/tmtc-c2a.log`

エラーが発生した場合は、これらのログファイルを確認してください。

### 注意事項

1. **データベースサイズ**: CSVデータの約20倍のサイズになります
   - 例: 70MBのCSV → 約1.6GBのDuckDB
   - 十分なディスク容量を確保してください

2. **インポート時間**: データ量に応じて数分かかる場合があります
   - 約17万レコード/分の処理速度
   - `--release`ビルドにより高速化

3. **必要なファイル**:
   - `tmtc-c2a/satconfig.json`
   - `tmtc-c2a/tlmcmddb.json`

### トラブルシューティング

#### gaia-recorderが起動しない

```bash
cat /tmp/gaia-recorder.log
```

#### tmtc-c2aが起動しない

```bash
cat /tmp/tmtc-c2a.log
```

#### データベースサイズが大きすぎる

手動で削除：
```bash
rm -f gaia-recorder/recordings/recording_*.duckdb
```

#### ポートが既に使用されている

既存のプロセスを終了：
```bash
# gaia-recorder (port 8920)
lsof -ti:8920 | xargs kill

# tmtc-c2a (port 8900)
lsof -ti:8900 | xargs kill
```
