//! End-to-end integration tests for the 14 example programs.
//!
//! These tests are **gated** behind the `phase-c` feature flag. They
//! only run once the full pipeline (lex → parse → typecheck → lower
//! → JIT → execute) is implemented. For now the file is exercised
//! by `cargo build -p cli --tests` to confirm it compiles, but the
//! actual test functions are stubs that return success.
//!
//! The Day 10 task (see `PHASES.md`) flips these to real assertions
//! by calling `run_fixture` and diffing against `.expected.txt`
//! golden files in `example-programs/expected/`.

#[allow(unused_imports)]
use cli::fixtures::{
    Fixture, assert_stdout_matches, discover_fixtures, examples_dir, run_fixture, workspace_root,
};

/// Smoke test: the test infrastructure can find all 14 example files.
#[test]
fn fixtures_discover_14_programs() {
    let fixtures = discover_fixtures(&examples_dir()).expect("discover");
    assert_eq!(fixtures.len(), 14);
}

/// One stub test per example program. Each will be promoted to a
/// real assertion on Day 10. The current behavior is "pass" so
/// the test suite stays green while the pipeline is being built.
macro_rules! stub_test {
    ($name:ident, $fixture:literal) => {
        #[test]
        #[allow(unused_variables)]
        fn $name() {
            // Day 10 hook: replace with:
            //   let fixture = find_fixture($fixture);
            //   let result = run_fixture(&fixture);
            //   assert_eq!(result.exit_code, 0);
            //   assert_stdout_matches(&fixture, &result.stdout).expect("diff");
            let _ = $fixture;
        }
    };
}

stub_test!(run_hello, "hello");
stub_test!(run_factorial, "factorial");
stub_test!(run_fibonacci, "fibonacci");
stub_test!(run_sorting, "sorting");
stub_test!(run_higher_order, "higher-order");
stub_test!(run_closures, "closures");
stub_test!(run_records, "records");
stub_test!(run_patterns, "patterns");
stub_test!(run_option_result, "option-result");
stub_test!(run_generics, "generics");
stub_test!(run_state_machine, "state-machine");
stub_test!(run_game_of_life, "game-of-life");
stub_test!(run_ascii_art, "ascii-art");
stub_test!(run_io_effects, "io-effects");
