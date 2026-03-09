// build.rs — CLI build script
//
// sqlx::migrate!("../migrations") embeds all migration files at compile time.
// Without this script, Cargo's incremental compilation has no way to know that
// a new .sql file was added, so it skips recompilation and the binary silently
// runs without the new migration embedded.
//
// This script declares each migration file (and the directory itself) as a
// build dependency. Cargo will rerun this script — and recompile the crate —
// whenever a file is added, removed, or modified in migrations/.

fn main() {
    // Watch the directory itself so that adding a new file triggers a rebuild.
    println!("cargo:rerun-if-changed=../migrations");

    // Watch each file individually so that editing an existing migration
    // (e.g. fixing a typo during development) also triggers a rebuild.
    if let Ok(entries) = std::fs::read_dir("../migrations") {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
}
