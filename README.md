# openparts-web

[OpenParts](https://github.com/openparts-org/openparts)のウェブサイト。プロジェクト紹介と、
`openparts-server` を使った部品検索・閲覧UIを兼ねます。

- ランディングページ(`/`)、KiCadプラグインの案内(`/kicad-plugin`)、コントリビュートガイド(`/contributing`)は静的コンテンツです。
- 検索(`/search`)とパーツ詳細(`/parts?manufacturer=...&mpn=...`)は、ブラウザ側からリクエスト時に `openparts-server` のREST APIへfetchします。ビルド時にデータを埋め込むわけではないので、サーバーのデータが更新されれば再ビルドなしで反映されます。

Astro + Preact(検索/詳細ページのみアイランドとしてハイドレート)、TypeScript、[bun](https://bun.sh)を使用しています。日本語(デフォルト、`/...`)と英語(`/en/...`)に対応しています。

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

   GitHub Pagesのプロジェクトサイトとして配信する前提で `astro.config.mjs` に `base: '/openparts-web'` を設定しているため、
   `http://localhost:4321/openparts-web/`(日本語)/ `http://localhost:4321/openparts-web/en/`(英語)を開いてください。

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
が同一オリジンで到達可能なAPIを指していない限り、実際のAPI疎通確認はできません。

## デプロイ(GitHub Pages)

`main` へのpushで `.github/workflows/deploy.yml` が自動的にビルドし、GitHub Pagesへデプロイします
(リポジトリの Settings > Pages で Source を "GitHub Actions" にしておく必要があります)。

`openparts-server` はまだどこにもデプロイされていないため、デプロイ直後は検索・部品詳細・3Dプレビュー・ダウンロードは
エラー表示になります(ランディング/KiCadプラグイン/コントリビュートページの静的コンテンツは問題なく表示されます)。
サーバーが立ったら、リポジトリの Settings > Secrets and variables > Actions > Variables で
`OPENPARTS_API_BASE` にAPIのベースURL(例: `https://api.openparts.example`)を設定し、
ワークフローを再実行してください。`openparts-server` にはCORSミドルウェアが無いので、
別オリジンにデプロイする場合はサーバー側にCORS対応を追加するか、同一オリジンのリバースプロキシを挟む必要があります。

## コントリビュート

部品データ(ピン配置・パッケージ寸法など)自体は `openparts-data` リポジトリが正本です。データの追加・修正方法は、このサイトの [`/contributing`](http://localhost:4321/contributing) ページを参照してください。
