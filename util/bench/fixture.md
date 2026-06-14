# Benchmark fixture

3 つの Mermaid 図を含む文書。gazu は 1 回の `render_stream` でまとめて
レンダリングするのに対し、mermaid-filter はブロックごとに `mmdc` (Puppeteer)
を起動するため、ブロック数に応じて差が広がる。

## Flowchart

```mermaid
flowchart TD
    A[Christmas] -->|Get money| B(Go shopping)
    B --> C{Let me think}
    C -->|One| D[Laptop]
    C -->|Two| E[iPhone]
    C -->|Three| F[fa:fa-car Car]
    D --> G[/OK/]
    E --> G
    F --> G
    G --> H{Enough budget?}
    H -->|Yes| I[Buy it]
    H -->|No| J[Keep saving]
    J --> B
    I --> K([Done])

    classDef green fill:#9f6,stroke:#333,stroke-width:2px
    classDef orange fill:#f96,stroke:#333,stroke-width:4px
    class I green
    class J orange
```

## Sequence diagram

```mermaid
sequenceDiagram
    actor User
    participant Browser
    participant BlogService
    participant AuthService
    participant DB

    User->>Browser: Visit post page
    Browser->>BlogService: GET /posts/123
    BlogService->>AuthService: Validate session token
    AuthService-->>BlogService: Session valid, user_id=42
    BlogService->>DB: SELECT * FROM posts WHERE id=123
    DB-->>BlogService: Post data
    BlogService->>DB: SELECT * FROM comments WHERE post_id=123
    DB-->>BlogService: Comments data
    BlogService-->>Browser: HTML response
    Browser-->>User: Render page

    User->>Browser: Submit comment
    Browser->>BlogService: POST /posts/123/comments
    BlogService->>AuthService: Validate session token
    AuthService-->>BlogService: Session valid, user_id=42
    BlogService->>DB: INSERT INTO comments ...
    DB-->>BlogService: OK
    BlogService-->>Browser: 201 Created
    Browser-->>User: Comment posted
```

## Git graph

```mermaid
gitGraph
    commit id: "init"
    commit id: "add README"

    branch develop
    checkout develop
    commit id: "setup project"
    commit id: "add auth module"

    branch feature/login
    checkout feature/login
    commit id: "login form"
    commit id: "validate input"
    commit id: "add tests"

    checkout develop
    merge feature/login id: "merge login"

    branch feature/signup
    checkout feature/signup
    commit id: "signup form"
    commit id: "email verification"

    checkout develop
    merge feature/signup id: "merge signup"
    commit id: "integration tests"

    checkout main
    merge develop id: "release v1.0" tag: "v1.0"

    checkout develop
    commit id: "start v1.1"
```
