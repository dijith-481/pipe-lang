// BFS Pathfinding Demo
//
// Demonstrates: functional BFS using arrays as queues,
// pattern matching, closures, fold/map/filter

let walls = [
    (0, 2), (0, 4), (0, 6),
    (1, 3), (1, 5), (1, 7),
    (2, 0), (2, 6),
    (3, 0), (3, 2), (3, 3), (3, 5), (3, 7),
    (4, 0), (4, 2),
    (5, 0), (5, 3), (5, 4), (5, 6),
    (6, 0), (6, 2), (6, 6),
    (7, 0), (7, 4),
]

let contains = (list, r, c) =>
    list.filter((p) => match p { (pr, pc) => pr == r && pc == c }).len() > 0usize

let is_wall = (r, c) => contains(walls, r, c)

let in_bounds = (r, c, rows, cols) =>
    r >= 0 && r < rows && c >= 0 && c < cols

let get_neighbors = (r, c) =>
    [(r - 1, c), (r + 1, c), (r, c - 1), (r, c + 1)]

let has_goal = (frontier, rows, cols) =>
    frontier.filter((p) => match p { (r, c) =>
        r == rows - 1 && c == cols - 1
    }).len() > 0usize

let bfs_loop = (frontier, visited, depth, rows, cols, remaining) => {
    match remaining {
        0 => (frontier, visited, depth)
        _ => {
            match frontier.len() {
                0usize => (frontier, visited, depth)
                _ => {
                    match has_goal(frontier, rows, cols) {
                        true => (frontier, visited, depth)
                        false => {
                            let next = frontier.fold([], (acc, cell) => match cell { (r, c) =>
                                concat(acc, get_neighbors(r, c).filter((n) => match n { (nr, nc) =>
                                    in_bounds(nr, nc, rows, cols) && !is_wall(nr, nc) && !contains(concat(visited, acc), nr, nc)
                                }))
                            })
                            let new_visited = concat(visited, frontier)
                            bfs_loop(next, new_visited, depth + 1, rows, cols, remaining - 1)
                        }
                    }
                }
            }
        }
    }
}

let label = (v) => v

let print_grid_cell = (visited, r, c) => {
    let ch = match is_wall(r, c) {
        true => `#`
        false => match contains(visited, r, c) { true => `.` false => ` ` }
    }
    let _ = label(print(ch))
    true
}

let print_grid_row = (visited, r, c, cols) => {
    match c < cols {
        true => {
            let _ = print_grid_cell(visited, r, c)
            print_grid_row(visited, r, c + 1, cols)
        }
        false => {
            let _ = label(println(``))
            true
        }
    }
}

let print_grid_all = (visited, r, rows, cols) => {
    match r < rows {
        true => {
            let _ = print_grid_row(visited, r, 0, cols)
            print_grid_all(visited, r + 1, rows, cols)
        }
        false => true
    }
}

let print_grid = (visited, rows, cols) => {
    let _ = print_grid_all(visited, 0, rows, cols)
    true
}

let main = () => {
    let rows = 8
    let cols = 8
    println(`=== BFS Pathfinding ===`)
    println(``)
    println(`Grid Legend: (space)=open, #=wall, .=visited`)
    println(``)
    println(`Start: (0, 0)`)
    println(`Goal:  (7, 7)`)
    println(``)
    let result = bfs_loop([(0, 0)], [], 0, 8, 8, 64)
    let (frontier, visited, depth) = match result { (a, b, c) => (a, b, c) }
    match has_goal(frontier, rows, cols) {
        true => {
            println(`Shortest path: ${to_str(depth)} steps from (0, 0) to (7, 7)`)
            println(``)
            println(`Grid (BFS exploration):`)
            let _ = print_grid(visited, rows, cols)
            println(``)
            println(`Reachable cells: ${to_str(visited.len())} / ${to_str(rows * cols)}`)
        }
        false => {
            println(`No path found from (0, 0) to (7, 7)`)
        }
    }
}
