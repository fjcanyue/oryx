# Mermaid theme showcase

Every diagram family, under every theme. This is the visual QA file:
after upgrading Merman, the theme system, or resvg, open this document
in Oryx and walk it once per theme in the Theme Browser.

What to look for, family by family:

- **Structure follows the theme's surfaces.** Node, actor, entity,
  class, state, requirement, cluster and note boxes paint the reading
  theme's own surface colors — never the link/syntax/alert accents.
- **Relationship and edge labels sit on a quiet ground.** The ER
  `contains` label and the requirement `satisfies` label paint the
  label surface, not the accent color.
- **Data figures keep their color.** Pie slices, git branches, XY and
  radar curves, timeline and journey sections draw with the theme's
  categorical series, so a themed document never bleaches its charts
  to grayscale.
- **Status keeps its meaning.** The Gantt critical task carries the
  danger color on its border over a readable tinted ground.
- **User styling wins.** The green node at the end of the flowchart
  section is green in every theme, because the author asked for it.

## Flowchart

```mermaid
flowchart LR
    A[Start] --> B{Valid?}
    B -->|Yes| C[Continue]
    B -->|No| D[Stop]
    style A fill:#00ff00
```

The first node is green in every theme: an explicit `style` is the
author's intent, and the theme adapter must not repaint it.

## State

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Running
    Running --> Done
    Done --> [*]
```

## Sequence — the actor regression

```mermaid
sequenceDiagram
    Alice->>Bob: Hello
    Bob-->>Alice: Hi
    Note over Alice: actors paint the surface
```

The actor boxes must never catch the theme's link accent — the bug
this showcase guards against.

## ER — the `contains` regression

```mermaid
erDiagram
    TABLE_A {
        varchar id PK
    }

    TABLE_B {
        varchar id PK
    }

    TABLE_A ||--o{ TABLE_B : contains
```

The `contains` relationship label paints the label surface. If it
ever shows the accent color (green in some themes), the
`tertiaryColor` contract has regressed.

## Class

```mermaid
classDiagram
    Animal <|-- Duck
    Duck : +swim()
```

## Requirement

```mermaid
requirementDiagram
    requirement req {
        id: 1
        text: the requirement
        risk: high
        verifymethod: analysis
    }

    element entity {
        type: simulation
    }

    entity - satisfies -> req
```

The `satisfies` relation label sits on the label surface, like ER's.

## Architecture

```mermaid
architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service server(server)[Server] in api

    server:L -- R:db
```

## C4

```mermaid
C4Context
    title System Context
    Person(customer, "Customer", "A customer")
    System(banking, "Banking System", "Keeps money")
    Rel(customer, banking, "Uses")
```

## Block

```mermaid
block-beta
    columns 3
    A["Source"]:3
    B["Left"] C["Right"]
    D["Result"]:3
    A-->B
    A-->C
    B-->D
    C-->D
```

## Kanban

```mermaid
kanban
    Todo
        id1[Task 1]
        id2[Task 2]
    In Progress
        id3[Task 3]
    Done
        id4[Task 4]
```

## Gantt — the status regression

```mermaid
gantt
    dateFormat YYYY-MM-DD
    section Section
    A task :a1, 2024-01-01, 30d
    Critical task :crit, 2024-01-01, 10d
    Done task :done, 2024-01-05, 5d
    Active task :active, after a1, 5d
```

The critical task carries the danger color; done tasks read as
alternate surface; task bars draw series colors with readable labels.

## Timeline

```mermaid
timeline
    title Project Timeline
    section Design
        2024-01 : Requirements : Draft
    section Build
        2024-02 : Implement : Test
```

## Journey

```mermaid
journey
    title My working day
    section Go to work
        Make tea: 5: Me
        Go upstairs: 3: Me
    section Work
        Do work: 8: Me
        Review: 4: Me
```

## GitGraph

```mermaid
gitGraph
    commit id: "INIT"
    branch develop
    commit id: "A"
    checkout main
    commit id: "B"
    merge develop
```

## Pie

```mermaid
pie title Tasks
    "Done" : 70
    "Todo" : 30
```

## XYChart

```mermaid
xychart-beta
    title "Sales"
    x-axis [jan, feb, mar]
    y-axis "Revenue" 0 --> 400
    bar [10, 30, 50]
    line [20, 40, 60]
```

## Radar

```mermaid
radar-beta
    title Senses
    axis leg["Legs"], vision["Vision"]
    curve cat["Cat"]{80, 60}
    curve dog["Dog"]{60, 40}
```

## Sankey

```mermaid
sankey-beta

A,B,10
B,C,5
A,C,7
```

## Treemap

```mermaid
treemap
    title "Disk usage"
    "docs" : 100
    "media" : 250
    "code" : 80
```

## Venn

```mermaid
venn-beta
    set A["Frontend"]
    set B["Backend"]
    union A,B["Shared APIs"]
```
