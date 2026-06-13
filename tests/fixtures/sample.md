# sekien-pandoc サンプル

## フローチャート

```mermaid
graph LR
  A[入力] --> B{分岐}
  B -- Yes --> C[処理]
  B -- No  --> D[スキップ]
  C --> E[出力]
  D --> E
```

## シーケンス図

```mermaid
sequenceDiagram
  participant U as User
  participant S as Server
  U->>S: リクエスト
  S-->>U: レスポンス
```

## 通常のコードブロック (変換しない)

```rust
fn main() {
    println!("Hello, world!");
}
```

## Div の中の Mermaid (再帰収集の確認)

::: note
```mermaid
graph TD
  X[Start] --> Y[End]
```
:::

## 壊れた Mermaid (graceful fallback の確認)

```mermaid
totallyBogusDiagram
```

## 通常テキスト

- 上の壊れた Mermaid はコードブロックのままHTMLに残るはず
- 他の図は SVG に変換されているはず
