//! Search index management.
//!
//! Usage:
//!   synap search reindex
//!
//! Note: Tantivy only allows one writer on the index directory at a time, and
//! the running server holds it open for its whole lifetime — so this only
//! works while the app is stopped (e.g. after a bulk import or DB restore
//! done offline). To reindex while the app is running, use the "Rebuild
//! Search Index" button under Settings → Advanced in the admin UI instead —
//! it reindexes in-process on the running server, no second writer needed.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum SearchAction {
    /// Rebuild the Tantivy search index from the database (all published
    /// posts/pages, every site). Only works while the app is stopped — the
    /// running server holds the only index writer allowed at a time. While
    /// the app is running, use the "Rebuild Search Index" button under
    /// Settings → Advanced in the admin UI instead.
    Reindex,
}

pub async fn run(action: SearchAction) -> anyhow::Result<()> {
    match action {
        SearchAction::Reindex => reindex().await,
    }
}

async fn reindex() -> anyhow::Result<()> {
    let cfg = synaptic_core::config::AppConfig::load()
        .map_err(|e| anyhow::anyhow!("Failed to load config: {e}"))?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&cfg.database_url)
        .await
        .map_err(|e| anyhow::anyhow!("Database connection failed: {e}\nCheck DATABASE_URL is correct and PostgreSQL is running."))?;

    let index = synaptic_core::search::SearchIndex::open_or_create(
        std::path::Path::new(&cfg.search_index_path),
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "Failed to open search index at '{}': {e}\n\n\
             Tantivy only allows one writer on the index at a time — if the app is \
             currently running, its server process is already holding it open. \
             Either stop the app first (this CLI command is for offline/deploy use, \
             e.g. after a bulk import with the app down), or, while the app is running, \
             use the \"Rebuild Search Index\" button under Settings → Advanced instead — \
             that reindexes in-process, so it doesn't need a second writer.",
            cfg.search_index_path
        )
    })?;

    println!("Rebuilding search index at '{}'...", cfg.search_index_path);

    match synaptic_core::search::indexer::rebuild_index(index, pool).await {
        Some(count) => {
            println!("Done — indexed {count} document(s).");
            Ok(())
        }
        None => anyhow::bail!("Reindex failed — check the logs above."),
    }
}
