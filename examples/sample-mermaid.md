# Mermaid diagrams

Every diagram kind Oryx renders, in one file. The renderer is Merman,
native Rust tracking mermaid.js 11.15; as a separate implementation,
the newest syntax forms may still differ from the JavaScript library.

## Flowchart

```mermaid
flowchart LR
    A[Start] --> B{Valid?}
    B -->|Yes| C[Continue]
    B -->|No| D[Stop]
```

## Sequence

```mermaid
sequenceDiagram
    Alice->>Bob: Hello
    Bob-->>Alice: Hi
```

## Class

```mermaid
classDiagram
    Animal <|-- Duck
```

## State

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Running
```

## ER

```mermaid
erDiagram
    USER ||--o{ ORDER : places
```

## Pie

```mermaid
pie title Tasks
    "Done" : 70
    "Todo" : 30
```

## Mindmap

```mermaid
mindmap
  root((Oryx))
    Read
    Edit
    Export
```

## Timeline

```mermaid
timeline
    title Project phases
    2024 Q1 : Design
    2024 Q2 : Build
    2024 Q3 : Ship
```

## Gantt

```mermaid
gantt
    title A small plan
    dateFormat YYYY-MM-DD
    section Work
    Draft      :a1, 2024-01-01, 10d
    Review     :after a1, 5d
```

## Chinese labels

```mermaid
flowchart LR
    A[读取文件] --> B[解析 Markdown]
    B --> C[生成 Mermaid]
    C --> D[显示图表]
```

## An invalid diagram

A bad block shows its error panel and the document reads on.

```mermaid
flowchart LR
    subgraph This subgraph never closes
    A --> B
```
