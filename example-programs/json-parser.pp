// JSON Parser — JSON value representation, construction, and querying
//
// Demonstrates: ADTs for recursive data structures, recursive pattern matching,
// higher-order functions, string templates for serialization, Option/Result

type Json =
  | JNull
  | JBool(bool)
  | JNum(f64)
  | JStr(str)
  | JArr(Array<Json>)
  | JObj(Array<(str, Json)>)

// -- Constructors --

let jNull = JNull
let jBool = (b) => JBool(b)
let jNum  = (n) => JNum(n)
let jStr  = (s) => JStr(s)
let jArr  = (items) => JArr(items)
let jObj  = (pairs) => JObj(pairs)

// -- Serialization --

let jsonToString = (json) => match json {
    JNull       => `null`
    JBool(b)    => match b { true => `true` false => `false` }
    JNum(n)     => to_str(n)
    JStr(s)     => `"${s}"`
    JArr(items) => {
        let parts = items.map(jsonToString)
        `[${parts.fold(``, (acc, s) => match acc { `` => s _ => `${acc}, ${s}` })}]`
    }
    JObj(pairs) => {
        let parts = pairs.map((p) => match p { (k, v) => `"${k}": ${jsonToString(v)}` })
        `{${parts.fold(``, (acc, s) => match acc { `` => s _ => `${acc}, ${s}` })}`
    }
}

// -- Query helpers --

let getField = (name, json) => match json {
    JObj(pairs) => {
        let matched = pairs.filter((p) => match p { (k, _) => k == name })
        matched.head().flat_map((p) => match p { (_, v) => Some(v) })
    }
    _ => None
}

let arrayGet = (idx, json) => match json {
    JArr(items) => match idx < items.len() {
        true  => Some(items[idx])
        false => None
    }
    _ => None
}

// -- Sample data: a blog post --

let blogPost = jObj([
    (`title`, jStr(`Introduction to Functional Programming`)),
    (`author`, jObj([
        (`name`, jStr(`Alice Johnson`)),
        (`email`, jStr(`alice@example.com`)),
        (`roles`, jArr([jStr(`author`), jStr(`editor`)]))
    ])),
    (`tags`, jArr([jStr(`fp`), jStr(`beginner`), jStr(`pipe-lang`)])),
    (`published`, jBool(true)),
    (`views`, jNum(1547.0)),
    (`metadata`, jObj([
        (`wordCount`, jNum(2500.0)),
        (`readingTime`, jNum(12.0)),
        (`rating`, jNum(4.5))
    ]))
])

// -- Sample data: API response --

let apiResponse = jObj([
    (`status`, jStr(`ok`)),
    (`code`, jNum(200.0)),
    (`data`, jArr([
        jObj([
            (`id`, jNum(1.0)),
            (`name`, jStr(`Widget A`)),
            (`price`, jNum(29.99)),
            (`inStock`, jBool(true))
        ]),
        jObj([
            (`id`, jNum(2.0)),
            (`name`, jStr(`Widget B`)),
            (`price`, jNum(49.99)),
            (`inStock`, jBool(false))
        ]),
        jObj([
            (`id`, jNum(3.0)),
            (`name`, jStr(`Widget C`)),
            (`price`, jNum(19.99)),
            (`inStock`, jBool(true))
        ])
    ]))
])

// -- Formatting helpers --

let printField = (name, json) => {
    let value = getField(name, json)
    match value {
        Some(v) => println(`  ${name}: ${jsonToString(v)}`)
        None    => println(`  ${name}: (not found)`)
    }
}

let printProduct = (product, idx) => {
    println(`  Product #${to_str(idx)}:`)
    printField(`id`, product)
    printField(`name`, product)
    printField(`price`, product)
    printField(`inStock`, product)
}

// -- Main --

let main = () => {
    println(`=== JSON Parser & Query ===`)
    println(``)

    println(`Blog Post (serialized):`)
    println(jsonToString(blogPost))
    println(``)

    println(`Queries:`)
    let title = getField(`title`, blogPost)
    match title {
        Some(t) => println(`  title = ${jsonToString(t)}`)
        None    => println(`  title not found`)
    }

    let author = getField(`author`, blogPost)
    match author {
        Some(a) => {
            let name = getField(`name`, a)
            match name {
                Some(n) => println(`  author name = ${jsonToString(n)}`)
                None    => println(`  author name not found`)
            }

            let email = getField(`email`, a)
            match email {
                Some(e) => println(`  author email = ${jsonToString(e)}`)
                None    => println(`  author email not found`)
            }
        }
        None => println(`  author not found`)
    }

    let views = getField(`views`, blogPost)
    match views {
        Some(v) => println(`  views = ${jsonToString(v)}`)
        None    => println(`  views not found`)
    }

    println(``)
    println(`API Response:`)
    println(jsonToString(apiResponse))
    println(``)

    let data = getField(`data`, apiResponse)
    match data {
        Some(JArr(products)) => {
            println(`Products (${to_str(products.len())} items):`)
            let indexed = products.fold([], (acc, p) => acc.concat([p]))
            indexed.map((p) => match p { JObj(_) => {
                let id = getField(`id`, p)
                let name = getField(`name`, p)
                let price = getField(`price`, p)
                match id {
                    Some(JNum(i)) => match name {
                        Some(JStr(n)) => match price {
                            Some(JNum(pr)) => println(`  #${to_str(i)} ${n}: $${to_str(pr)}`)
                            _ => println(`  unknown`)
                        }
                        _ => println(`  unknown`)
                    }
                    _ => println(`  unknown`)
                }
            } _ => println(`  unknown`) })
        }
        _ => println(`  no data found`)
    }

    println(``)
    println(`Nested query: author email from blog post`)
    let email = getField(`author`, blogPost).flat_map((a) => getField(`email`, a))
    match email {
        Some(JStr(e)) => println(`  Result: ${e}`)
        _             => println(`  Not found`)
    }
}
