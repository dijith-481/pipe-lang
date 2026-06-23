# pipe-lang Standard Library Reference v0.1

## Prelude (auto-imported)

All prelude functions are available in every pipe-lang program without an import statement.

### Core Combinators

| Function | Signature | Description |
|----------|-----------|-------------|
| `id` | `<A>(x: A) -> A` | Returns its argument unchanged |
| `const` | `<A, B>(x: A, _: B) -> A` | Ignores second argument, returns first |
| `flip` | `<A, B, C>(f: (A, B) -> C) -> (B, A) -> C` | Swaps the first two arguments |
| `compose` | `<A, B, C>(f: (B) -> C, g: (A) -> B) -> (A) -> C` | Right-to-left composition: `compose(f, g)(x) = f(g(x))` |
| `pipe` | `<A, B, C>(f: (A) -> B, g: (B) -> C) -> (A) -> C` | Left-to-right composition: `pipe(f, g)(x) = g(f(x))` |
| `apply` | `<A, B>(f: (A) -> B, x: A) -> B` | Applies function to argument |

### IO Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `println` | `(value: str) -> Effect<()>` | Prints value followed by newline to stdout |
| `print` | `(value: str) -> Effect<()>` | Prints value to stdout without newline |
| `read_line` | `() -> Effect<str>` | Reads one line from stdin (includes trailing newline) |
| `read_file` | `(path: str) -> Effect<Result<str, str>>` | Reads file contents; returns `Ok(content)` or `Err(msg)` |

### Numeric Conversions

| Function | Signature | Description |
|----------|-----------|-------------|
| `to_i32` | `(f64) -> i32` | Truncates f64 to i32 |
| `to_i64` | `(i32) -> i64` | Widens i32 to i64 |
| `to_f64` | `(i32) -> f64` | Converts i32 to f64 |
| `to_str` | various | Formats value as string |

---

## Array Methods

Available on any `Array<T>` value via method syntax: `arr.map(f)`.

### `map`
```
map<A, B>(array: Array<A>, f: (A) -> B) -> Array<B>
```
Returns a new array where every element is transformed by `f`.

### `filter`
```
filter<T>(array: Array<T>, pred: (T) -> bool) -> Array<T>
```
Returns a new array containing only elements where `pred` returns `true`.

### `fold`
```
fold<A, B>(array: Array<A>, initial: B, f: (B, A) -> B) -> B
```
Left fold. Reduces the array to a single value by applying `f` cumulatively.

```
[1, 2, 3].fold(0, (acc, x) => acc + x)   // → 6
```

### `flat_map`
```
flat_map<A, B>(array: Array<A>, f: (A) -> Array<B>) -> Array<B>
```
Maps each element to an array, then flattens the result.

```
[[1, 2], [3]].flat_map((x) => x)  // → [1, 2, 3]
```

### `concat`
```
concat<T>(left: Array<T>, right: Array<T>) -> Array<T>
```
Returns a new array containing all elements from `left` followed by all from `right`.

### `prepend`
```
prepend<T>(array: Array<T>, value: T) -> Array<T>
```
Returns a new array with `value` added at the front.

### `len`
```
len<T>(array: Array<T>) -> usize
```
Returns the number of elements in the array.

### `head`
```
head<T>(array: Array<T>) -> Option<T>
```
Returns `Some(first_element)` or `None` if the array is empty.

### `tail`
```
tail<T>(array: Array<T>) -> Option<Array<T>>
```
Returns `Some(all_elements_except_first)` or `None` if the array is empty.

---

## Option Methods

Available on any `Option<T>` value.

### `map`
```
map<A, B>(opt: Option<A>, f: (A) -> B) -> Option<B>
```
Transforms the inner value if `Some`, returns `None` unchanged.

### `flat_map`
```
flat_map<A, B>(opt: Option<A>, f: (A) -> Option<B>) -> Option<B>
```
Chains optional operations. Returns `None` if any step returns `None`.

### `unwrap_or`
```
unwrap_or<A>(opt: Option<A>, default: A) -> A
```
Returns the inner value if `Some`, otherwise returns `default`.

---

## Result Methods

Available on any `Result<T, E>` value.

### `map`
```
map<T, E, U>(result: Result<T, E>, f: (T) -> U) -> Result<U, E>
```
Transforms the `Ok` value, passes `Err` through unchanged.

### `flat_map`
```
flat_map<T, E, U>(result: Result<T, E>, f: (T) -> Result<U, E>) -> Result<U, E>
```
Chains fallible operations. Short-circuits on `Err`.

---

## String Methods

Available on any `str` value.

### `len`
```
len(s: str) -> usize
```
Returns the byte length of the string (not character count).

### `concat`
```
concat(left: str, right: str) -> str
```
Returns a new string with `right` appended to `left`.

### `split`
```
split(s: str, delimiter: str) -> Array<str>
```
Splits the string by `delimiter`. Returns an array of substrings.

```
"a,b,c".split(",")   // → ["a", "b", "c"]
```

### `trim`
```
trim(s: str) -> str
```
Returns a new string with leading and trailing whitespace removed.

### `parse_i32`
```
parse_i32(s: str) -> Result<i32, str>
```
Attempts to parse the string as an i32. Returns `Ok(value)` or `Err(msg)`.

---

## IO Module (`use stdlib::io`)

Requires an explicit import. Provides the same IO functions as the prelude, scoped under the `io` module:

```
use stdlib::io

let main = () =>
    read_line()
        .flat_map((line) => println(`You typed: ${line}`))
```

| Function | Signature | Description |
|----------|-----------|-------------|
| `io.println` | `(str) -> Effect<()>` | Same as prelude `println` |
| `io.read_line` | `() -> Effect<str>` | Same as prelude `read_line` |
| `io.read_file` | `(str) -> Effect<Result<str, str>>` | Same as prelude `read_file` |
