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

// Parse a CSV line into fields using split
let parse_csv = (line) => {
    let fields = line.split(`,`)
    match fields.len() >= 5usize {
        true => {
            let qty = fields[3usize].parse_i32().unwrap_or(0)
            let prc = fields[2usize].parse_i32().unwrap_or(0)
            Some((fields[0usize], fields[1usize], to_f64(prc), qty, fields[4usize]))
        }
        false => None
    }
}

// Valid rows
let unwrap_or_empty = (r) => match r { Some(v) => v _ => (``, ``, 0.0, 0, ``) }
let rows = data.map(parse_csv).filter((r) => match r { Some(_) => true _ => false }).map(unwrap_or_empty)

let prod  = (r) => match r { (p, _, _, _, _) => p }
let cat   = (r) => match r { (_, c, _, _, _) => c }
let prc   = (r) => match r { (_, _, p, _, _) => p }
let qty   = (r) => match r { (_, _, _, q, _) => q }
let cty   = (r) => match r { (_, _, _, _, c) => c }
let rev   = (r) => prc(r) * to_f64(qty(r))

let row_str = (r) => `${prod(r)} | ${cat(r)} | $${to_str(prc(r))} | ${to_str(qty(r))} | ${cty(r)}`

// Filter helpers
let by_cat = (rows, c) => rows.filter((r) => cat(r) == c)
let by_cty = (rows, c) => rows.filter((r) => cty(r) == c)

let sum_rev = (rows) => rows.fold(0.0, (s, r) => s + rev(r))

let avg_prc = (rows) => match to_f64(rows.len()) > 0.0 {
    true  => rows.fold(0.0, (s, r) => s + prc(r)) / to_f64(rows.len())
    false => 0.0
}

// Unique values from a column
let unique = (rows, f) =>
    rows.fold([], (acc, r) => {
        let val = f(r)
        // to_str creates a fresh string copy, avoiding a JIT bug where
        // TagGet results are corrupted when passed to array_literal.
        let fresh = to_str(val)
        if acc.filter((x) => x == fresh).len() > 0usize {
            acc
        } else {
            acc.concat([fresh])
        }
    })

// Top N by price (workaround: fold instead of take, which has a pre-existing bug)
let top_n = (rows, n) =>
    rows.fold([], (acc, r) => [r].concat(acc)).fold([], (acc2, r) => {
        if acc2.len() < n { acc2.concat([r]) } else { acc2 }
    })

// Format helpers
let print_title = (t) => { println(``); println(`=== ${t} ===`) }

// -- Main --
let main = () => {
    println(`=== CSV Query Tool ===`)
    println(`${to_str(rows.len())} rows loaded`)

    print_title(`All data`)
    rows.map(row_str).map((s) => println(s))

    let cats = unique(rows, cat)

    print_title(`Revenue by category`)
    cats.map((c) => println(`  ${c}: $${to_str(sum_rev(by_cat(rows, c)))}`))

    print_title(`Average price by category`)
    cats.map((c) => println(`  ${c}: $${to_str(avg_prc(by_cat(rows, c)))}`))

    print_title(`Top 5 products`)
    top_n(rows, 5usize).map((r) => println(`  ${prod(r)}: $${to_str(prc(r))} (${cty(r)})`))

    print_title(`Electronics in New York`)
    by_cty(by_cat(rows, `Electronics`), `New York`)
        .map((r) => println(`  ${prod(r)}: $${to_str(prc(r))} x ${to_str(qty(r))}`))

    print_title(`City breakdown`)
    let cities = unique(rows, cty)
    cities.map((c) => {
        let items = by_cty(rows, c)
        println(`  ${c}: ${to_str(items.len())} items, $${to_str(sum_rev(items))} revenue`)
    })

    println(``)
    println(`Processed ${to_str(rows.len())} rows across ${to_str(cats.len())} categories.`)
}
