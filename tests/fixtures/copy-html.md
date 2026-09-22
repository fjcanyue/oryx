---
title: Copy as HTML
description: Every construct Oryx renders, in one file to copy and paste
---

# Copy as HTML

This file holds every construct Oryx renders. Select all, copy, and paste into an email, a Word document or a web editor to see how each one comes through.

## Headings

### Heading 3

#### Heading 4

##### Heading 5

###### Heading 6

## Paragraphs and line breaks

First paragraph, with a line ending in two spaces  
and one in a backslash\
before a plain line ending
that joins.

Second paragraph. Smart punctuation: "quotes", 'quotes', dashes -- and ---, ellipsis...

<p align="center">A centered paragraph.</p>

<p align="right">A paragraph on the right.</p>

## Inline styles

**bold**, *italic*, ***bold italic***, ~~strikethrough~~, `inline code`, <u>underline</u>, <ins>inserted</ins>, <mark>highlighted</mark>, ==also highlighted==, <small>small</small>, <kbd>Ctrl</kbd>.

H~2~O and CO~2~, E = mc^2^ and x^10^, x<sup>2</sup> and H<sub>2</sub>O.

Shortcodes: :tada: :rocket: :warning: and pasted emoji ☕ 🚀

Entities: &lt; &gt; &amp; &quot; &copy; &mdash; &eacute; &#x1F600;

## Links

[a link](https://example.com), [a section link](#headings), [a file link](README.md), a bare URL https://example.com/~user/page, <someone@example.com>, and [a reference link][ref].

[ref]: https://example.com

## Lists

- unordered item
- another, with **bold** and a [link](https://example.com)
  - nested one level
    - nested two levels
- back at the top

1. ordered item
2. ordered item
   1. nested ordered item
   2. and another
3. third item

A sentence between two lists, or markdown joins them.

7. a list starting at seven
8. its second item

- an item with a second paragraph

  the second paragraph of the item

- the next item

- [ ] open task
- [x] done task

Oryx
: A fast viewer and editor for markdown, code and books.

Markdown
: A plain text format that reads well as written.
: Also the name of the tool that first converted it.

## Quotes and alerts

> a quote, with *italic* and `code` in it
>
> > nested one level

> [!NOTE]
> The five GitHub alert kinds render with their own color and title.

> [!TIP]
> A tip.

> [!IMPORTANT]
> Something important.

> [!WARNING]
> A warning.

> [!CAUTION]
> A caution.

## Code

```rust
fn fenced() -> &'static str {
    // a comment
    "highlighted when the language is recognized"
}
```

```python
def tilde_fences(n: int) -> str:
    return "work the same" * n
```

    an indented block renders as code too
    with a second line

```
a fence with no language
```

```diff
- a removed line
+ an added line
```

## Tables

| Left | Center | Right |
|:-----|:------:|------:|
| a    | b      | c     |
| long cells wrap | stripes alternate | columns size to content |
| **bold** | `code` | [link](https://example.com) |

<table>
  <caption>A caption renders centered above</caption>
  <tr><td>no header at all</td><td>no header band</td></tr>
</table>

## Images

![a local image](../../examples/oryx-test.png)

An inline image in a sentence: <img src="../../examples/oryx-test.png" width="48"> and a broken one: ![missing picture](missing.png).

## Footnotes and abbreviations

A claim with a footnote,[^note] and another.[^1] The specification comes from the W3C, and RTL text reads right to left.

[^1]: The definition gathers at the foot of the document.

[^note]: This one is used first, so it shows as 1.

    A second paragraph of the same footnote.

*[W3C]: World Wide Web Consortium
*[RTL]: right to left

## Math

Inline math: $e^{i\pi} + 1 = 0$, or fenced: $`a^2 + b^2 = c^2`$, and a price of $5-$10 that stays text.

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
$$

```math
\begin{pmatrix} a & b \\ c & d \end{pmatrix} \quad \sqrt[3]{x^3+y^3}
```

## Rules and breaks

Text above a rule.

---

Text below the rule and above a page break.

<div style="page-break-after: always"></div>

Text after the page break.

## Collapsible sections

<details open>
<summary>An open section</summary>

Its content, a paragraph.

</details>

<details>
<summary>A closed section</summary>

Hidden content.

</details>

## The end

The last paragraph of the file.
