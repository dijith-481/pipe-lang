//! End-to-end integration tests for all example programs.

use cli::fixtures::{Fixture, assert_stdout_matches, discover_fixtures, examples_dir, run_fixture};

fn get_fixture(name: &str) -> Fixture {
    let fixtures = discover_fixtures(&examples_dir()).expect("discover fixtures");
    fixtures
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("fixture `{name}` not found"))
}

fn assert_fixture_ok(name: &str) {
    let fixture = get_fixture(name);
    let result = run_fixture(&fixture);
    assert_eq!(
        result.exit_code, 0,
        "{} failed: stderr={}",
        name, result.stderr
    );
    assert_stdout_matches(&fixture, &result.stdout).unwrap_or_else(|e| panic!("{e}"));
}

// ---------------------------------------------------------------------------
// Working programs
// ---------------------------------------------------------------------------

#[ignore = "Dijith: implement run_fixture in cli/src/fixtures.rs"]
#[test]
fn e2e_hello() {
    assert_fixture_ok("hello");
}

// ---------------------------------------------------------------------------
// Programs blocked on Member 1 (JIT instructions)
// ---------------------------------------------------------------------------

#[ignore = "Member 1: implement MakeClosure + CallNamed for recursive calls"]
#[test]
fn e2e_factorial() {
    assert_fixture_ok("factorial");
}

#[ignore = "Member 1: implement MakeClosure + CallNamed"]
#[test]
fn e2e_fibonacci() {
    assert_fixture_ok("fibonacci");
}

#[ignore = "Member 1: implement ArrayAlloc/Get/Set + recursive calls"]
#[test]
fn e2e_sorting() {
    assert_fixture_ok("sorting");
}

#[ignore = "Member 1: implement MakeClosure + CallIndirect + Arrays"]
#[test]
fn e2e_higher_order() {
    assert_fixture_ok("higher-order");
}

#[ignore = "Member 1: implement MakeClosure + CallIndirect"]
#[test]
fn e2e_closures() {
    assert_fixture_ok("closures");
}

#[ignore = "Member 1: implement RecordAlloc/Get/Set"]
#[test]
fn e2e_records() {
    assert_fixture_ok("records");
}

#[ignore = "Member 1: implement TagConstruct/Discriminant/Get"]
#[test]
fn e2e_patterns() {
    assert_fixture_ok("patterns");
}

#[ignore = "Member 1: implement TagConstruct + Member 2: Type dispatch"]
#[test]
fn e2e_option_result() {
    assert_fixture_ok("option-result");
}

#[ignore = "Member 1: implement MakeClosure + CallIndirect"]
#[test]
fn e2e_generics() {
    assert_fixture_ok("generics");
}

#[ignore = "Member 1: implement TagConstruct + Array builtins"]
#[test]
fn e2e_state_machine() {
    assert_fixture_ok("state-machine");
}

#[ignore = "Member 1: implement ArrayAlloc + ArrayGet + ArraySet"]
#[test]
fn e2e_game_of_life() {
    assert_fixture_ok("game-of-life");
}

#[ignore = "Member 1: implement CallNamed + StrConcat"]
#[test]
fn e2e_ascii_art() {
    assert_fixture_ok("ascii-art");
}

// ---------------------------------------------------------------------------
// Program blocked on Member 2 (IO module + Effect runtime)
// ---------------------------------------------------------------------------

#[ignore = "Member 2: implement stdlib::io module resolution + Effect runtime"]
#[test]
fn e2e_io_effects() {
    assert_fixture_ok("io-effects");
}

// ---------------------------------------------------------------------------
// NEW programs (blocked on full JIT + typechecker)
// ---------------------------------------------------------------------------

#[ignore = "Full pipeline: ADTs + recursion"]
#[test]
fn e2e_expression_evaluator() {
    assert_fixture_ok("expression-evaluator");
}

#[ignore = "Full pipeline: arrays + methods + tuples"]
#[test]
fn e2e_csv_query() {
    assert_fixture_ok("csv-query");
}

#[ignore = "Full pipeline: ADTs + recursion + nested data"]
#[test]
fn e2e_json_parser() {
    assert_fixture_ok("json-parser");
}

#[ignore = "Full pipeline: arrays + tuples + recursion"]
#[test]
fn e2e_pathfinding_bfs() {
    assert_fixture_ok("pathfinding-bfs");
}

#[ignore = "Full pipeline: effects + IO + recursion"]
#[test]
fn e2e_tiny_repl() {
    assert_fixture_ok("tiny-repl");
}

#[ignore = "Full pipeline: string processing + ADTs"]
#[test]
fn e2e_markdown_renderer() {
    assert_fixture_ok("markdown-renderer");
}

// ---------------------------------------------------------------------------
// Smoke: fixture infrastructure loads all programs
// ---------------------------------------------------------------------------

#[test]
fn e2e_discover_all_20_programs() {
    let fixtures = discover_fixtures(&examples_dir()).expect("discover");
    assert_eq!(fixtures.len(), 20, "expected 20 example programs");
}
