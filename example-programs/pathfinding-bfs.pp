// Pathfinding (BFS) — grid-based shortest path search
//
// Demonstrates: recursion at module level, array operations, higher-order
// functions, functional queue (via arrays), pure algorithm implementation

// Grid: 0 = open, 1 = wall
// Start: (0, 0), Goal: (7, 7)
let grid = [
    [0, 0, 0, 0, 1, 0, 0, 0],
    [0, 1, 1, 0, 1, 0, 1, 0],
    [0, 0, 0, 0, 0, 0, 1, 0],
    [0, 1, 0, 1, 1, 0, 1, 0],
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 1, 0, 1, 0],
    [0, 1, 0, 0, 0, 0, 1, 0],
    [0, 0, 0, 0, 1, 0, 0, 0]
]

let rows = 8
let cols = 8

// Cell value at (r, c)
let cell = (r, c) => grid[r][c]

let inBounds = (r, c) => r >= 0 && r < rows && c >= 0 && c < cols
let isOpen = (r, c) => inBounds(r, c) && cell(r, c) == 0

// Four cardinal directions
let neighbors = (r, c) =>
    [(r-1, c), (r+1, c), (r, c-1), (r, c+1)]
        .filter((n) => match n { (nr, nc) => isOpen(nr, nc) })

// Check if a position is in a list
let contains = (list, r, c) =>
    list.filter((p) => match p { (pr, pc) => pr == r && pc == c }).len() > 0

// -- BFS (top-level recursion) --
// Invariant: queue is a list of positions to explore
// visited is a list of already-explored positions
// BFS: returns Some(depth) if goal reached, None if unreachable
let bfsStep = (queue, visited, depth) => {
    match queue.len() {
        0 => None  // queue empty = unreachable
        _ => {
            let h = queue.head()
            match h {
                Some((r, c)) => {
                    let t = queue.tail()
                    match t {
                        Some(q) => {
                            match r == 7 && c == 7 {
                                true  => Some(depth)
                                false => {
                                    let added = visited.concat([(r, c)])
                                    let next = neighbors(r, c).filter((n) =>
                                        match n { (nr, nc) => !contains(added, nr, nc) }
                                    )
                                    bfsStep(q.concat(next), added, depth + 1)
                                }
                            }
                        }
                        None => None
                    }
                }
                None => None
            }
        }
    }
}

let bfs = (sr, sc) => bfsStep([(sr, sc)], [], 0)

// -- Grid display --
let displayGrid = (visited) => {
    // Row 0
    println(`${gridChar(0, 0, visited)}${gridChar(0, 1, visited)}${gridChar(0, 2, visited)}${gridChar(0, 3, visited)}${gridChar(0, 4, visited)}${gridChar(0, 5, visited)}${gridChar(0, 6, visited)}${gridChar(0, 7, visited)}`)
    println(`${gridChar(1, 0, visited)}${gridChar(1, 1, visited)}${gridChar(1, 2, visited)}${gridChar(1, 3, visited)}${gridChar(1, 4, visited)}${gridChar(1, 5, visited)}${gridChar(1, 6, visited)}${gridChar(1, 7, visited)}`)
    println(`${gridChar(2, 0, visited)}${gridChar(2, 1, visited)}${gridChar(2, 2, visited)}${gridChar(2, 3, visited)}${gridChar(2, 4, visited)}${gridChar(2, 5, visited)}${gridChar(2, 6, visited)}${gridChar(2, 7, visited)}`)
    println(`${gridChar(3, 0, visited)}${gridChar(3, 1, visited)}${gridChar(3, 2, visited)}${gridChar(3, 3, visited)}${gridChar(3, 4, visited)}${gridChar(3, 5, visited)}${gridChar(3, 6, visited)}${gridChar(3, 7, visited)}`)
    println(`${gridChar(4, 0, visited)}${gridChar(4, 1, visited)}${gridChar(4, 2, visited)}${gridChar(4, 3, visited)}${gridChar(4, 4, visited)}${gridChar(4, 5, visited)}${gridChar(4, 6, visited)}${gridChar(4, 7, visited)}`)
    println(`${gridChar(5, 0, visited)}${gridChar(5, 1, visited)}${gridChar(5, 2, visited)}${gridChar(5, 3, visited)}${gridChar(5, 4, visited)}${gridChar(5, 5, visited)}${gridChar(5, 6, visited)}${gridChar(5, 7, visited)}`)
    println(`${gridChar(6, 0, visited)}${gridChar(6, 1, visited)}${gridChar(6, 2, visited)}${gridChar(6, 3, visited)}${gridChar(6, 4, visited)}${gridChar(6, 5, visited)}${gridChar(6, 6, visited)}${gridChar(6, 7, visited)}`)
    println(`${gridChar(7, 0, visited)}${gridChar(7, 1, visited)}${gridChar(7, 2, visited)}${gridChar(7, 3, visited)}${gridChar(7, 4, visited)}${gridChar(7, 5, visited)}${gridChar(7, 6, visited)}${gridChar(7, 7, visited)}`)
}

let gridChar = (r, c, visited) =>
    match (cell(r, c), contains(visited, r, c)) {
        (0, true)  => `.`
        (0, false) => ` `
        (1, _)     => `#`
        _          => `?`
    }

// -- Main --
let main = () => {
    println(`=== BFS Pathfinding ===`)
    println(``)
    println(`Grid Legend: (space)=open, #=wall, .=visited`)
    println(``)
    println(`Start: (0, 0)`)
    println(`Goal:  (7, 7)`)
    println(``)

    let steps = bfs(0, 0)
    match steps {
        None => println(`No path found! The goal is unreachable.`)
        Some(n) => println(`Shortest path: ${to_str(n)} steps from (0, 0) to (7, 7)`)
    }

    println(``)
    println(`Grid (. = open, # = wall):`)
    println(` .##.# # `)
    println(` #  # # #`)
    println(` #     # `)
    println(` # ## # #`)
    println(` # #     `)
    println(`  ## #   `)
    println(` # #   # `)
    println(`   #     `)
    displayGrid([])
    displayGrid(visited)
    println(``)
    println(`Reachable cells: ${to_str(visited.len())} / ${to_str(rows * cols)}`)
}
