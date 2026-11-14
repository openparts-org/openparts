//! Registers every `templates/*.yaml` as a `Fragment` (see
//! `src/template.rs`): adding a template file is all it takes.

use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("templates/ exists")
        .map(|e| e.expect("readable templates/ entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let list: String = names
        .iter()
        .map(|n| format!("    fragment!({n:?}),\n"))
        .collect();
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("templates.rs");
    std::fs::write(out, format!("&[\n{list}]\n")).unwrap();
}
