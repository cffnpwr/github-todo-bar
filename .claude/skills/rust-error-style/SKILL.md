---
name: rust-error-style
description: |
  Encodes Rust error-type design conventions for this project. Use when (1) defining a new error enum, (2) extending an existing error enum with new variants, (3) reviewing PRs that touch error types, (4) deciding error variant granularity or naming, (5) wiring up source chains via thiserror, (6) deciding what `From` implementations to provide, (7) refactoring error handling for traceability or UI surfacing, (8) designing errors for code that calls external systems (HTTP, DB, filesystem) and needs to surface failure context. Covers: thiserror as the standard library-layer crate, variant naming without enum-name stutter, separation of error variants by failure source, including failure context (status, body, etc.) in variants, source-chain construction via `#[from]` and `#[source]`, when to use `#[from]` versus manual From, Display formatting guidelines.
compatibility: No external dependencies. Works in all environments with standard AI Agent tools.
---

# Rust Error Style

このスキルは Rust エラー型定義の規約をまとめたもの。新規実装・既存修正・PR レビューのいずれでも、このスキルが示す形に揃える。

## 1. クレート選定

- ライブラリ層は `thiserror` を使う
- アプリケーション層は `anyhow` / `eyre` を使ってもよい
- `thiserror` を本体で使う場合は main deps に置く

## 2. 命名

| 規約 | 良い例 | 悪い例 |
|---|---|---|
| enum 名は `<Domain>Error` | `ParseError`、`ConfigError` | `Errors`、`Err` |
| variant 名は短い名詞 | `Http`、`Connection`、`Parse`、`NotFound` | `HttpError`、`FailedToParse`、`ConnectionFailed` |
| enum 名と stutter しない | `ParseError::Syntax` | `ParseError::SyntaxError` |
| 動詞句にしない | `Parse` | `FailedToParse`、`CouldNotConnect` |

参考: `std::io::ErrorKind::{NotFound, PermissionDenied, ...}`、`serde_json::error::Category::{Io, Syntax, Data, Eof}`。

## 3. variant の粒度

**外部要因の種別ごとに variant を分ける**。「種別」は呼び出し側が異なる対応を取る単位:

- ログメッセージの分類だけが違う → 同じ variant でよい
- 呼び出し側が「再試行する/しない」「ユーザーに別画面を出す」「再認証を促す」等の判断を変えるべき → variant を分ける

variant が細かすぎると `match` 側がバージョンアップで壊れやすく、粗すぎると呼び出し側で分岐するために中身を覗く必要が出る。**呼び出し側の意思決定単位** を基準に粒度を決める。

### 外部 API クライアント等での参考区分

外部 HTTP API を叩くクライアントなら、最低限以下を分離する典型例:

- **認証失敗**(HTTP 401 等): re-auth フローを駆動するため独立必須
- **接続失敗**(transport: DNS / TCP / TLS / timeout): UI で「ネットワーク確認」誘導が必要
- **レスポンスのパース失敗**: パースライブラリのエラー詳細(行/列)が出るので独立
- **その他のサーバ側エラー**: ステータスコード等を持って保持
- **ドメイン検証失敗**: パースは通るがアプリのルールに違反するケース(必須フィールドの値が範囲外、URL の host が想定外、等)

## 4. 失敗コンテキストを variant に含める

外部システム由来のエラーでは、デバッグや UI 表示に有用なコンテキストを variant のフィールドに保持する。

- HTTP エラーなら `status: u16` と `body: String`
- DB エラーなら クエリ識別子や rowid
- パースエラーなら ファイル名や位置(ライブラリの `Error` から取れる場合は source 経由でもよい)

```rust
#[error("HTTP error: status {status}, body: {body}")]
Http {
    status: u16,
    body: String,
    #[source]
    source: reqwest::Error,
},
```

センシティブ情報(認証トークンの値等)は保持しない。返ってきたレスポンス body はサーバが提供している情報なので OK。

## 5. source chain(トレース可能性)

すべての variant について、内包する原因エラーがあれば `Error::source()` で辿れるようにする。`thiserror` の `#[from]` または `#[source]` 属性を必ず付ける。

```rust
#[derive(Debug, Error)]
pub(crate) enum MyError {
    #[error("authentication failed, body: {body}")]
    Unauthorized {
        body: String,
        #[source]
        source: SomeExternalError,
    },

    #[error("HTTP error: status {status}, body: {body}")]
    Http {
        status: u16,
        body: String,
        #[source]
        source: SomeExternalError,
    },

    #[error("connection failure")]
    Connection(#[from] SomeExternalError),

    #[error("parse failure")]
    Parse(#[from] ParseLibraryError),

    #[error("invalid value: {0}")]
    InvalidValue(String),
}
```

- `#[from]` は1 enum につき同じ source 型を1 variant にしか付けられない。`?` 経由で「自動的にこの variant にしたい」という1 種類だけに使う
- `#[source]` は手動で構築する variant のうち source を保持したいものに付ける。フィールド名が `source` なら属性省略可だが、明示推奨
- ドメイン側エラー(`InvalidValue(String)` 等)は通常 source なしで OK。message 自体がトレース情報

## 6. From 実装の方針

- 1 variant が `?` 経由で構築される自然なルートでは `#[from]` を使う
- 1 つの source 型を複数 variant に振り分ける必要がある場合(例: 同じエラー型をステータスコードによって認証失敗/通常 HTTP エラー/接続失敗に振り分ける)は、From は最も汎用な variant 1 つだけにし、それ以外は実装コード側で明示的に構築する
- `From<T>` で複雑な分岐ロジック(status による分岐、特定値の検出等)を持たせない。フィールドが空の variant が `?` で勝手に作られる落とし穴になる

## 7. Display フォーマット

`#[error("...")]` の文字列は次の指針で書く:

- 状況 + 主要パラメータの形(`HTTP error: status 500, body: ...`)
- enum 名や variant 名は重ねて書かない(`Display` で `MyError::Http: HTTP error...` のような stutter を避ける)
- センシティブ情報(認証トークン等)は出さない
- source の Display は別途展開される(anyhow / eyre / 自前ロガーが chain を辿るため、自分の `#[error]` 文字列に source の中身を埋め込まない)

## 8. テスト連携

エラー variant が外部 crate のエラー型(`reqwest::Error` 等)を内包すると `PartialEq` を derive できない。テストでは `assert!(matches!(err, Variant { ... }))` で variant + リテラルフィールド検証する前提で variant を設計する。具体的なテスト規約は `rust-test-style` スキル参照。

## チェックリスト

新規にエラーを追加するとき、または既存を修正するときに次を満たすこと:

- [ ] `thiserror` 使用(library 層の場合)
- [ ] enum 名 `<Domain>Error`、variant 名は短い名詞、stutter しない
- [ ] variant の粒度は呼び出し側の意思決定単位に合わせる
- [ ] 外部システム由来エラーには有用な失敗コンテキスト(status/body/identifier 等)を保持
- [ ] 各 variant の source chain が `#[from]` または `#[source]` で繋がっている
- [ ] `From<T>` は単純な1対1変換に限定。分岐ロジックを持たない
- [ ] Display は状況 + 主要パラメータ。stutter しない、センシティブ情報を出さない
