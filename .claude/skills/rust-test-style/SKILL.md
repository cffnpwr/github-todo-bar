---
name: rust-test-style
description: |
  Encodes Rust unit-test conventions for this project. Use when (1) writing new tests in any module, (2) modifying existing tests, (3) reviewing PRs that touch test code, (4) refactoring tests for consistency, (5) deciding test naming, function structure, or assertion patterns, (6) writing tests for code that calls external systems (HTTP, DB, filesystem) and needs mocks, (7) deciding fixture or helper organization. Covers: AAA (Arrange-Act-Assert) pattern with mandatory block comments, `positive_` / `negative_` prefix on test function names, one test case per function (no parameterization or loops), exact-match `assert_eq!` for success paths, `matches!` for error paths with literal pattern fields, structured fixture macros, helper extraction patterns. Includes additional rules for HTTP client tests using wiremock.
compatibility: No external dependencies. Works in all environments with standard AI Agent tools.
---

# Rust Test Style

このスキルは Rust 単体テストの規約をまとめたもの。新規実装・既存修正・PR レビューのいずれでも、このスキルが示す形に揃える。

## 1. ファイル配置と attribute

- 単体テストは対象モジュールと同じファイル内に `#[cfg(test)] mod tests { ... }` で書く
- 同期テストは `#[test]`、非同期テストは `#[tokio::test]`(`tokio` の `macros` + 必要に応じ `rt-multi-thread` features を有効化)
- 外部システムを叩く SUT を持つ場合は対応するモック手段を使う(HTTP なら `wiremock`、DB なら test container 等)

## 2. テスト関数名

`<positive|negative>_<subject>_<verb>_<expected>` の形に統一する。

- 正常系には `positive_` を必ず付ける
- 異常系には `negative_` を必ず付ける
- `subject` は SUT のメソッド名や対象機能名
- `verb_expected` は何を返すか、どの状態になるか、どの variant を返すか

例:
```
positive_<method>_returns_<expected_value>
positive_<method>_returns_empty_for_<input_state>
negative_<method>_returns_<error_variant>_on_<trigger>
```

## 3. 1 ケース 1 関数

- パラメタライズ・ループ・サブテスト等で 1 関数に複数ケースを詰めない
- 似たケース(例: 複数の HTTP ステータスコード)もそれぞれ独立した関数として書く
- 同じ事象でも別 entry point(別メソッド)なら別関数

## 4. AAA パターン(コメント必須)

各関数は次の3ブロックに分け、ブロック直前に `// Arrange` / `// Act` / `// Assert` をリテラルに書く。

```rust
#[test]
fn positive_<name>() {
    // Arrange
    // - 入力データ構築 / モック設定 / SUT 構築 / 期待値の準備
    let input = ...;
    let sut = ...;
    let expected = ...;

    // Act
    // - SUT の呼び出し1行のみ。unwrap せず Result<_, _> のまま受ける
    let result = sut.do_something(input);

    // Assert
    // - ここで unwrap / unwrap_err してから比較
    assert_eq!(result.unwrap(), expected);
}
```

- Arrange に「期待値 (`expected`)」も含める
- Act は SUT 呼び出しの1行のみ。`.expect()` / `.unwrap()` は含めない
- Assert で `unwrap()` / `unwrap_err()` を行う

## 5. 検証パターン

| 種別 | 手段 | 理由 |
|---|---|---|
| 成功時の戻り値 | `assert_eq!(result.unwrap(), expected)` で完全一致 | ドメイン型は `Debug + PartialEq + Eq` を derive すれば完全一致比較できる |
| 失敗時のエラー | `assert!(matches!(result.unwrap_err(), MyError::Variant { .. }))` | エラー variant が外部 crate のエラー(`reqwest::Error` 等)を内包する場合、`PartialEq` を満たせない。variant 一致と公開フィールド(コード等)のリテラル一致を確認する |
| エラーの構造体フィールド | `matches!(_, MyError::Http { status: 403, .. })` のようにリテラル | プリミティブ値はリテラルで完全一致確認、source などその他フィールドは `..` で無視 |

成功系テストでは事前に Arrange ブロックで `expected` を構築し、Assert で `assert_eq!` する。

## 6. ドメイン型の derive 要件

`assert_eq!` で完全一致する以上、テスト対象の戻り値型は次を必ず derive する:

```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MyOutput { ... }
```

- `Debug`: `assert_eq!` の失敗メッセージで使う
- `PartialEq` + `Eq`: 比較
- `Hash` は不要(必要になった時に追加)

浮動小数点等で `Eq` が付けられない場合は `PartialEq` のみでよい(その時点で完全一致が成立しないので `assert_eq!` の使い方を要検討)。

## 7. fixture の書き方

データ形式に対応する宣言マクロを使い、生文字列は使わない。

- JSON: `serde_json::json!`
- TOML: `toml::toml!` (適宜)
- その他: 当該ライブラリの構築マクロ / builder

```rust
let body = json!({
    "field": "value",
    "items": [...]
});
```

ライブラリを利用する関係でテストから直接参照する crate は **dev-deps** に置く。本体実装でも使うものは main deps に置き、テストでは継承する。

## 8. テストヘルパ

繰り返し使う構築は `tests` モジュール先頭に常数とヘルパ関数で集約する:

```rust
const TEST_INPUT: &str = "...";

fn build_sut(config: ...) -> Sut {
    Sut::new(config)
}
```

- 動的ヘルパは `fn`、定数は `const` で表現
- 定数の文字列連結等が必要なら `const_format::concatcp!` を使う(dev-dep)

## 9. HTTP クライアントのテスト追加ルール

外部 HTTP API を叩くクライアントの単体テストには `wiremock` を使う。

### MockServer 起動と Client 構築

クライアントは base URL を差し替えられる構造にしておき、テストでは `MockServer::start()` の URI を渡す:

```rust
let server = MockServer::start().await;
let client = build_client(&server.uri());  // base URL 差し替え用コンストラクタ
```

### リクエスト形状の strict 検証

正常系の HTTP テストでは、wiremock のマッチャでリクエスト全体を strict 検証する。マッチャに合わなければ wiremock がデフォルトで 404 を返し、テストが失敗する仕組み。

正常系で必ず検証するもの:
- `method(...)` / `path(...)`
- `query_param(...)` でクエリ全項目
- `header(...)` で必須ヘッダ全部(`Authorization` / `User-Agent` / `Accept` / API バージョンヘッダ等)
- `.expect(1)` で呼び出し回数を1回に固定

```rust
Mock::given(method("GET"))
    .and(path("/resource"))
    .and(query_param("filter", "value"))
    .and(header("Authorization", EXPECTED_AUTH))
    .and(header("User-Agent", EXPECTED_USER_AGENT))
    .respond_with(ResponseTemplate::new(200).set_body_json(&body))
    .expect(1)
    .mount(&server)
    .await;
```

異常系では `method` + `path` のみで十分(リクエスト形状は正常系で押さえているため)。

### HTTP ステータス別エラーテスト

ステータスコードごとに独立した関数で書く(`negative_<method>_returns_<variant>_on_<status>`)。`Mock` の `respond_with(ResponseTemplate::new(<status>))` で各ステータスを返し、対応するエラー variant が返ることを `matches!` で検証。

## チェックリスト

新規にテストを追加するとき、または既存を修正するときに次を満たすこと:

### 共通
- [ ] `positive_` または `negative_` プレフィックス
- [ ] 1 ケース 1 関数
- [ ] `// Arrange` / `// Act` / `// Assert` のコメント
- [ ] Act は SUT 呼び出し1行(`unwrap` なし)
- [ ] Assert で `unwrap()` / `unwrap_err()`
- [ ] 成功系は `assert_eq!` で完全一致
- [ ] 失敗系は `matches!` で variant + リテラルフィールド検証
- [ ] fixture は適切な宣言マクロ(`json!` 等)
- [ ] 比較対象のドメイン型に `Debug + PartialEq + Eq`

### HTTP クライアントテスト追加分
- [ ] `MockServer::start()` 経由でモックサーバ
- [ ] base URL 差し替えコンストラクタを SUT に用意
- [ ] 正常系は method/path/query/headers すべてを matcher で strict 検証
- [ ] `.expect(1)` で呼び出し回数を固定
