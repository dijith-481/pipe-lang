// Markdown to HTML Renderer — parse simple markdown and render HTML
//
// Demonstrates: string processing, ADTs for document model, pattern matching,
// template literals for HTML generation, array transformations

// Document model
type Inline =
  | Text(str)
  | Bold(str)
  | Code(str)
  | Link(str, str)  // (text, url)

type Block =
  | Heading(i32, str)
  | Paragraph(Array<Inline>)
  | CodeBlock(str, str)
  | ListItem(str)
  | Hr

type Doc = Array<Block>

// -- Sample markdown document --
let sampleDoc = [
    Heading(1, `Welcome to pipe-lang`),
    Paragraph([
        Text(`pipe-lang is a `),
        Bold(`functional`),
        Text(` programming language with `),
        Code(`Effect<T>`),
        Text(` for side effects.`)
    ]),
    Heading(2, `Features`),
    Paragraph([
        Text(`Here are some key features:`)
    ]),
    ListItem(`Hindley-Milner type inference`),
    ListItem(`Algebraic data types with pattern matching`),
    ListItem(`First-class functions and closures`),
    ListItem(`Effect system for pure/impure separation`),
    ListItem(`JIT compilation via Cranelift`),
    Hr,
    Heading(2, `Code Example`),
    CodeBlock(`pipe-lang`, `
let factorial = (n) => match n {
    0 => 1
    1 => 1
    n => n * factorial(n - 1)
}

let main = () => println(factorial(5))
`),
    Paragraph([
        Text(`The `),
        Code(`factorial`),
        Text(` function demonstrates recursion and pattern matching.`)
    ]),
    Heading(3, `More Information`),
    Paragraph([
        Text(`Visit the `),
        Link(`documentation`, `https://pipe-lang.dev/docs`),
        Text(` for more details.`)
    ])
]

// -- Renderers --

let escapeHtml = (s) =>
    s.split(`&`).fold(``, (acc, p) => match acc { `` => p _ => `${acc}&amp;${p}` })
        .split(`<`).fold(``, (acc, p) => match acc { `` => p _ => `${acc}&lt;${p}` })
        .split(`>`).fold(``, (acc, p) => match acc { `` => p _ => `${acc}&gt;${p}` })

let renderInline = (inline) => match inline {
    Text(t)      => escapeHtml(t)
    Bold(t)      => `<strong>${escapeHtml(t)}</strong>`
    Code(t)      => `<code>${escapeHtml(t)}</code>`
    Link(t, u)   => `<a href="${escapeHtml(u)}">${escapeHtml(t)}</a>`
}

let renderInlines = (inlines) =>
    inlines.map(renderInline).fold(``, (acc, s) => `${acc}${s}`)

let renderBlock = (block) => match block {
    Heading(1, t) => `<h1>${escapeHtml(t)}</h1>`
    Heading(2, t) => `<h2>${escapeHtml(t)}</h2>`
    Heading(3, t) => `<h3>${escapeHtml(t)}</h3>`
    Heading(n, t) => `<h${to_str(n)}>${escapeHtml(t)}</h${to_str(n)}>`
    Paragraph(inlines) => `<p>${renderInlines(inlines)}</p>`
    CodeBlock(lang, code) => `<pre><code class="language-${escapeHtml(lang)}">${escapeHtml(code)}</code></pre>`
    ListItem(text) => `<li>${escapeHtml(text)}</li>`
    Hr => `<hr>`
}

let renderDoc = (doc) => {
    let body = doc.map(renderBlock).fold(``, (acc, s) => `${acc}\n${s}`)
    `<!DOCTYPE html>
<html>
<head>
  <title>pipe-lang Documentation</title>
  <style>
    body { font-family: sans-serif; max-width: 800px; margin: 0 auto; padding: 2em; }
    code { background: #f0f0f0; padding: 2px 4px; border-radius: 3px; }
    pre { background: #f5f5f5; padding: 1em; border-radius: 5px; overflow-x: auto; }
    hr { border: none; border-top: 1px solid #ccc; margin: 2em 0; }
  </style>
</head>
<body>${body}
</body>
</html>`
}

// -- Stats --
let countBlocks = (doc) => doc.len()

let countInlines = (doc) =>
    doc.fold(0, (acc, block) => match block {
        Paragraph(inlines) => acc + inlines.len()
        _ => acc
    })

// -- Main --
let main = () => {
    println(`=== Markdown to HTML Renderer ===`)
    println(``)
    println(`Document stats:`)
    println(`  Blocks: ${to_str(countBlocks(sampleDoc))}`)
    println(`  Inlines in paragraphs: ${to_str(countInlines(sampleDoc))}`)
    println(``)
    println(`=== Generated HTML ===`)
    println(renderDoc(sampleDoc))
}
