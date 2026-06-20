// CSV Query Tool — read and query CSV data functionally
//
// Demonstrates: array methods (map, filter, fold), string processing,
// closures, Option/Result, pure data transformation pipelines

// Sample sales data: Product, Category, Price, Quantity, City
let data = [
    `Laptop,Electronics,1200,5,New York`,
    `Phone,Electronics,800,12,New York`,
    `Tablet,Electronics,450,8,Boston`,
    `Shirt,Clothing,40,30,Chicago`,
    `Jeans,Clothing,80,20,Chicago`,
    `Shoes,Clothing,120,15,Boston`,
    `Desk,Furniture,350,3,New York`,
    `Chair,Furniture,200,10,Boston`,
    `Lamp,Furniture,60,25,Chicago`,
    `Monitor,Electronics,500,7,New York`,
    `Keyboard,Electronics,100,20,Boston`,
    `Mouse,Electronics,40,35,Chicago`
]

// Parse a CSV line into a 5-tuple: (product, category, price, quantity, city)
let parseRow = (line) => {
    let cols = line.split(`,`)
    match cols.len() {
        5 => {
            let qty = match cols[3].parse_i32() { Ok(n) => n Err(_) => 0 }
            let prc = match cols[2].parse_i32() { Ok(n) => to_f64(n) Err(_) => 0.0 }
            Some((cols[0], cols[1], prc, qty, cols[4]))
        }
        _ => None
    }
}

// Valid rows
let unwrap = (r) => match r { Some(v) => v _ => (``, ``, 0.0, 0, ``) }
let rows = data.map(parseRow).filter((r) => match r { Some(_) => true None => false }).map(unwrap)

let prod  = (r) => match r { (p, _, _, _, _) => p }
let cat   = (r) => match r { (_, c, _, _, _) => c }
let prc   = (r) => match r { (_, _, p, _, _) => p }
let qty   = (r) => match r { (_, _, _, q, _) => q }
let cty   = (r) => match r { (_, _, _, _, c) => c }
let rev   = (r) => prc(r) * to_f64(qty(r))

let rowStr = (r) => `${prod(r)} | ${cat(r)} | $${to_str(prc(r))} | ${to_str(qty(r))} | ${cty(r)}`

// Filter helpers
let byCat = (rows, c) => rows.filter((r) => cat(r) == c)
let byCty = (rows, c) => rows.filter((r) => cty(r) == c)

let sumRev = (rows) => rows.fold(0.0, (s, r) => s + rev(r))

let avgPrc = (rows) => match to_f64(rows.len()) > 0.0 {
    true  => rows.fold(0.0, (s, r) => s + prc(r)) / to_f64(rows.len())
    false => 0.0
}

// Unique values from a column
let unique = (rows, f) =>
    rows.fold([], (acc, r) => {
        let val = f(r)
        match acc.filter((x) => x == val).len() > 0 {
            true  => acc
            false => acc.concat([val])
        }
    })

// Top N by price (simple: prepend + take, unsorted but shows top items)
let topN = (rows, n) =>
    rows.fold([], (acc, r) => [r].concat(acc)).take(n)

// Format helpers
let printTitle = (t) => { println(``); println(`=== ${t} ===`) }

// -- Main --
let main = () => {
    println(`=== CSV Query Tool ===`)
    println(`${to_str(rows.len())} rows loaded`)

    printTitle(`All data`)
    rows.map(rowStr).map((s) => println(s))

    let cats = unique(rows, cat)

    printTitle(`Revenue by category`)
    cats.map((c) => println(`  ${c}: $${to_str(sumRev(byCat(rows, c)))}`))

    printTitle(`Average price by category`)
    cats.map((c) => println(`  ${c}: $${to_str(avgPrc(byCat(rows, c)))}`))

    printTitle(`Top 5 products`)
    topN(rows, 5).map((r) => println(`  ${prod(r)}: $${to_str(prc(r))} (${cty(r)})`))

    printTitle(`Electronics in New York`)
    byCty(byCat(rows, `Electronics`), `New York`)
        .map((r) => println(`  ${prod(r)}: $${to_str(prc(r))} x ${to_str(qty(r))}`))

    printTitle(`City breakdown`)
    let cities = unique(rows, cty)
    cities.map((c) => {
        let items = byCty(rows, c)
        println(`  ${c}: ${to_str(items.len())} items, $${to_str(sumRev(items))} revenue`)
    })

    println(``)
    println(`Processed ${to_str(rows.len())} rows across ${to_str(cats.len())} categories.`)
}
