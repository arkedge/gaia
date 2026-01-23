# Example Files

このディレクトリには、gaia-recorderの動作確認・評価用のファイルが含まれています。

## ファイル一覧

- `tlmcmddb.json` - テレメトリ・コマンド定義データベース（スキーマ定義）
- `satconfig.json` - 衛星設定ファイル（tmtc-c2a用、パケットルーティング設定）
- `c2a-4la-mobc-sils` - C2A MOBC SILSシミュレータバイナリ
- `spaghetti.sh` - SILS用テレメトリ生成スクリプト
- `spaghetti.sils.jsonnet` - SILS用設定ファイル（jsonnet形式）

## 使用方法

### 記録モードでの起動（SILSと連携）

```bash
# ルートディレクトリから実行
pnpm start
```

これにより以下が起動します:
- gaia-recorder (記録サーバー)
- tmtc-c2a (テレメトリ・コマンドプロキシ)
- kble (SILSテレメトリ読み取り)
- c2a-4la-mobc-sils (SILSシミュレータ)

### プレイバックモードでの起動

```bash
# ルートディレクトリから実行
pnpm run playback
```

過去の記録データを閲覧できます（新規記録は行われません）。

## 注意事項

- これらのファイルは評価・テスト用であり、本番環境での使用を想定していません
- `c2a-4la-mobc-sils` は特定のハードウェア構成に依存するバイナリです
- `tlmcmddb.json` と `satconfig.json` は対象のC2Aシステムに合わせて調整してください
