// Markdown to HTML Renderer — parse simple markdown and render HTML
//
// Demonstrates: string processing, ADTs for document model, pattern matching,
// template literals for HTML generation, array transformations

type Inline =
  | T(str)
  | B(str)
  | C(str)
  | L(str, str)

type Block =
  | H1(str)
  | H2(str)
  | H3(str)
  | P(str)
  | Pre(str, str)
  | Li(str)
  | Hr

let esc : (str) -> str = (s) =>
    s.split(`&`).fold(``, (a, p) => match a { `` => p _ => `${a}&amp;${p}` })
        .split(`<`).fold(``, (a, p) => match a { `` => p _ => `${a}&lt;${p}` })
        .split(`>`).fold(``, (a, p) => match a { `` => p _ => `${a}&gt;${p}` })

let renderInline : (Inline) -> str = (i) => match i {
    T(t)    => esc(t)
    B(t)    => `<strong>${esc(t)}</strong>`
    C(t)    => `<code>${esc(t)}</code>`
    L(t, u) => `<a href="${esc(u)}">${esc(t)}</a>`
}

let renderBlock : (Block) -> str = (b) => match b {
    H1(t)     => `<h1>${esc(t)}</h1>`
    H2(t)     => `<h2>${esc(t)}</h2>`
    H3(t)     => `<h3>${esc(t)}</h3>`
    P(t)      => `<p>${esc(t)}</p>`
    Pre(l, c) => `<pre><code class="language-${esc(l)}">${esc(c)}</code></pre>`
    Li(t)     => `<li>${esc(t)}</li>`
    Hr        => `<hr>`
}

let main = () => {
    println(`=== HTML Output ===`)
    println(renderBlock(H1(`Welcome`)))
    println(renderBlock(P(`A functional language with pattern matching.`)))
    println(renderBlock(H2(`Features`)))
    println(renderBlock(Li(`Type inference`)))
    println(renderBlock(Li(`First-class functions`)))
    println(renderBlock(Li(`JIT compilation`)))
    println(renderBlock(Hr))
    println(renderBlock(Pre(`pipe-lang`, `let x = 42\nprintln(x)`)))
}
