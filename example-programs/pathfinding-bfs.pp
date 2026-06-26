// BFS Pathfinding Demo
//
// Demonstrates: functional BFS using arrays as queues,
// pattern matching, closures, fold/map/filter

// Check if position is valid
let in_bounds = (r, c, rows, cols) =>
    r >= 0 && r < rows && c >= 0 && c < cols

// Check if a position is in a list
let contains = (list, r, c) =>
    list.filter((p) => match p { (pr, pc) => pr == r && pc == c }).len() > 0usize

// Four cardinal directions
let get_neighbors = (r, c) =>
    [(r - 1, c), (r + 1, c), (r, c - 1), (r, c + 1)]

// BFS step: expand frontier by one level
let bfs_step = (frontier, visited, depth, rows, cols) => {
    match frontier.len() {
        0usize => None
        _ => {
            let current = match frontier.head() { Some(v) => v _ => (0, 0) }
            let rest = match frontier.tail() { Some(t) => t _ => [] }
            let r = match current { (cr, _) => cr }
            let c = match current { (_, cc) => cc }

            match r == rows - 1 && c == cols - 1 {
                true => Some(depth)
                false => {
                    let new_visited = visited.concat([current])
                    let neighbors = get_neighbors(r, c)
                    let valid = neighbors.filter((n) => match n { (nr, nc) =>
                        in_bounds(nr, nc, rows, cols) && !contains(new_visited, nr, nc)
                    })
                    bfs_step(rest.concat(valid), new_visited, depth + 1, rows, cols)
                }
            }
        }
    }
}

let bfs = (rows, cols) => bfs_step([(0, 0)], [], 0, rows, cols)

// -- Main --
let main = () => {
    println(`=== BFS Pathfinding ===`)
    println(``)
    println(`BFS explores a graph level by level.`)
    println(`Starting from (0, 0), expanding to neighbors.`)
    println(``)
    println(`For a 4x4 grid:`)
    println(`  . . . .`)
    println(`  . . . .`)
    println(`  . . . .`)
    println(`  . . . G`)
    println(``)
    println(`BFS guarantees shortest path in unweighted graphs.`)
    println(``)
    println(`Done.`)
}
