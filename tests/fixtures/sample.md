# gazu sample

## Flowchart

```mermaid
graph LR
  A[Input] --> B{Branch}
  B -- Yes --> C[Process]
  B -- No  --> D[Skip]
  C --> E[Output]
  D --> E
```

## Sequence diagram

```mermaid
sequenceDiagram
  participant U as User
  participant S as Server
  U->>S: Request
  S-->>U: Response
```

## Regular code block (not converted)

```rust
fn main() {
    println!("Hello, world!");
}
```

## Mermaid inside a Div (checks recursive collection)

::: note
```mermaid
graph TD
  X[Start] --> Y[End]
```
:::

## Broken Mermaid (checks graceful fallback)

```mermaid
totallyBogusDiagram
```

## Regular text

- The broken Mermaid above should remain as a code block in the HTML
- The other diagrams should be converted to SVG
