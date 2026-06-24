fn main() {
    let src_dir = std::path::Path::new("../../src");
    if !src_dir.exists() {
        println!("cargo:warning=tree-sitter-pipe-lang: grammar not generated yet; run `npx tree-sitter generate` first");
        return;
    }
    cc::Build::new()
        .include(src_dir)
        .file(src_dir.join("parser.c"))
        .file(src_dir.join("scanner.c"))
        .compile("tree-sitter-pipe-lang");
}
