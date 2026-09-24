# openparts-web

[OpenParts](https://github.com/openparts-org/openparts)のウェブサイト。プロジェクト紹介と、
`openparts-server` を使った部品検索・閲覧UIを兼ねます。

- ランディングページ(`/`)、KiCadプラグインの案内(`/kicad-plugin`)、コントリビュートガイド(`/contributing`)は静的コンテンツです。
- 検索(`/search`)とパーツ詳細(`/parts?manufacturer=...&mpn=...`)は、ブラウザ側からリクエスト時に `openparts-server` のREST APIへfetchします。ビルド時にデータを埋め込むわけではないので、サーバーのデータが更新されれば再ビルドなしで反映されます。

Astro + Preact(検索/詳細ページのみアイランドとしてハイドレート)、TypeScript、[bun](https://bun.sh)を使用しています。

## ローカル開発

このリポジトリは `openparts-server` / `openparts-data` と兄弟ディレクトリに配置されている前提です:

```
openparts/
├── openparts-web/      (このリポジトリ)
├── openparts-server/
└── openparts-data/
```

1. まずAPIサーバーを起動します(別ターミナル):

   ```sh
   cd ../openparts-server
   OPENPARTS_DATA_DIR=../openparts-data cargo run
   ```

   `http://localhost:8080` で待ち受けます。`curl localhost:8080/health` が `ok` を返せばOKです。

2. このリポジトリで依存関係をインストールして開発サーバーを起動します:

   ```sh
   bun install
   bun dev
   ```

   `http://localhost:4321` を開いてください。

`openparts-server` にはCORSミドルウェアが無いため、`astro.config.mjs` のVite devプロキシで
`/api/*` を `http://localhost:8080/*` に転送し、ブラウザからは常に同一オリジンで通信します(`openparts-server`
側は変更していません)。APIのベースURLは `PUBLIC_OPENPARTS_API_BASE` 環境変数で上書きできます(`.env.example` 参照、デフォルトは `/api`)。

## ビルド

```sh
bun build
bun preview
```

**注意**: Viteのdevプロキシは `bun dev` にのみ効きます。`bun build && bun preview`
は静的ビルド出力をそのまま配信するため、`PUBLIC_OPENPARTS_API_BASE`
が同一オリジンで到達可能なAPIを指していない限り、実際のAPI疎通確認はできません
(本番デプロイ構成は未定のため、今回のスコープには含めていません)。

## コントリビュート

部品データ(ピン配置・パッケージ寸法など)自体は `openparts-data` リポジトリが正本です。データの追加・修正方法は、このサイトの [`/contributing`](http://localhost:4321/contributing) ページを参照してください。
