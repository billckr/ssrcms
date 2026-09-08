---
title: Search
group: feature
updated_by: claude
last_updated: 2026-08-22
---
# Search

> Last updated: 2026-08-22 | Updated by: claude

## Overview

Full-text search is powered by Tantivy, an embedded Rust search engine (no external Elasticsearch dependency). The index is rebuilt from all published posts on startup and kept in sync on every publish, update, or delete operation. Search queries are capped at 25 characters. Results are filtered by `site_id` to enforce multisite isolation.

## How It Works

### Index (`core/src/search/index.rs`)

`SearchIndex` wraps a Tantivy `Index` with a thread-safe `Arc<RwLock<IndexWriter>>`. The schema has six fields: `id` (STRING STORED), `site_id` (STORED), `title` (indexed+stored), `content` (indexed only), `slug` (STORED), `post_type` (STRING STORED).

A custom tokenizer chain named `"en_stop"` is registered on the index: `SimpleTokenizer` → `LowerCaser` → `StopWordFilter` (removes ~70 common English stop words, defined in the `EN_STOP_WORDS` static) → `Stemmer(English)`. This is applied at both index time and query time.

Key methods:
- `open_or_create` — opens existing index or creates new; detects schema mismatch (e.g. tokenizer changed) and wipes/recreates the index directory
- `search(query_str, site_id, limit)` — parses query via `QueryParser` on title+content fields; falls back to `parse_query_lenient` on parse errors (special characters like `+`, `-`, `:`); ORs in an as-you-type prefix match on the last word (see below); fetches `limit * 4 + 20` docs when `site_id` filtering is needed, then post-filters by `site_id`; returns `Vec<SearchResult>` (id, title, slug, post_type, score)
- `rebuild_all` — deletes all documents, batch inserts, single commit (used for startup rebuild — avoids one disk flush per document)
- `upsert` — delete-by-id-term then add document, commit
- `delete` — delete by id term, commit

Tantivy only allows a single `IndexWriter` to hold the index directory's lockfile at a time (in the same process or a different one). `SearchIndex::open_or_create` acquires that writer up front and holds it for the caller's whole lifetime — this is why a second process (e.g. the CLI, see below) can't open the index while the server is already running.

### Indexer (`core/src/search/indexer.rs`)

- `index_post` — strips HTML via `ammonia::clean_text`, calls `index.upsert`
- `delete_post` — calls `index.delete`
- `rebuild_index(index, pool) -> Option<usize>` — async function that fetches all posts with `status = 'published'` from the DB and calls `index.rebuild_all`; returns the number of documents indexed, or `None` on failure. Runs as a background task on startup so it doesn't block server start with large post counts, and is also callable on demand (see On-Demand Reindex below) — added 2026-08-21 so a full rebuild no longer requires waiting for the next process start.

### On-Demand Reindex (added 2026-08-21)

Two ways to trigger `rebuild_index` outside of startup, for content added or changed outside the normal admin handlers (a WordPress import, a seed script, a direct DB write) that would otherwise stay unsearchable until the next restart:

- **Admin UI** — Settings → Advanced → "Search Index" card → "Rebuild Search Index" button (`super_admin` only). Calls `POST /admin/settings/dev-tools/reindex-search` (`core/src/handlers/admin/dev_tools.rs::reindex_search`), which clones the running server's own `Arc<SearchIndex>`/`PgPool` from `AppState` and awaits `rebuild_index` in-process — no second writer involved, so it works anytime the app is up. Returns `{"ok": true, "indexed": <count>}` as JSON, shown in the card.
- **CLI** — `synap search reindex` (`cli/src/commands/search.rs`). Loads `AppConfig` (for `database_url` and `search_index_path`), opens its own `PgPool` and `SearchIndex`, and calls `rebuild_index`. Because of the single-writer constraint above, this **only works while the app is stopped** — if the server is running, `open_or_create` fails with a Tantivy `LockBusy` error, and the CLI surfaces a message pointing at the admin UI button as the live-app alternative. Intended for offline/scripted use (e.g. immediately after a bulk import or DB restore performed with the app down).

### As-You-Type Prefix Matching (added 2026-08-22)

Because the index stores stemmed terms, an exact-word search requires typing (or stemming down to) the whole word — e.g. matching "Advanced" required typing all the way to its stem `advanc`, since neither the query parser nor the index does substring/prefix matching. `search()` now also extracts the last whitespace-separated word of the query (`last_token_prefix`: lowercased, punctuation stripped, `None` if under 2 chars) and ORs a `tantivy::query::PhrasePrefixQuery` for that word into the result, one per searched field (title, content), alongside the normal parsed query.

This needed no schema change, no new field, and no reindex: a `PhrasePrefixQuery` built from a single term degrades internally to a `RangeQuery` over the term dictionary bounded by the prefix bytes (see tantivy's `PhrasePrefixQuery::weight` — the phrase-adjacency path only applies when 2+ terms are supplied), which is a cheap FST prefix walk capped at the default 50-term expansion, not a full-index scan. It also still matches correctly against the *stemmed* dictionary without stemming the query itself: English suffix-stripping stemmers never modify the front of a word, so a lowercased raw prefix of the user's in-progress word remains a valid byte-prefix of the stemmed dictionary term it will eventually complete into (e.g. `"adv"` is a valid prefix of the stem `"advanc"`).

This was chosen over indexing edge n-grams for real substring autocomplete — n-grams give true substring matching but multiply the indexed token count (and therefore index size and rebuild time) roughly by average word length; the prefix-query approach adds negligible per-query cost with zero storage/indexing overhead, at the cost of only matching from the start of a word, not the middle. Covered by unit tests in `core/src/search/index.rs`'s `mod tests`.

### Search Handler (`core/src/handlers/search.rs`)

`GET /search?q=...` — enforces the 25-character query limit server-side (mirrors the HTML input's `maxlength`), calls `state.search_index.search(&query, Some(&site_id_str), 20)`, then fetches full `Post` records from the DB by the returned IDs (re-verifying `status == "published"` before including them) and builds a `PostContext` per result via `build_post_context`. Renders `search.html` with `query`, `results`, and `result_count` context variables, plus the standard nav/session/site context. Increments the `synaptic_search_queries_total` metric on non-empty queries. Renders active-plugin hook outputs (`head_start`, `head_end`, `body_start`, `body_end`, `before_content`, `after_content`, `footer`) for the resolved theme before returning HTML.

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | /search | `search::search` | Full-text search |
| POST | /admin/settings/dev-tools/reindex-search | `dev_tools::reindex_search` | Rebuild the search index on demand (`super_admin` only) |

## Configuration

`search_index_path` in `AppConfig` — path to the Tantivy index directory (default: `search-index`).

## Known Limitations / TODOs

The index does not support phrase queries or exact-match strings out of the box. Stop-word-only searches (e.g. "the") return zero results by design. Schema changes require a full index rebuild. Posts created outside the normal admin handlers (seed scripts, direct SQL, imports) are no longer stuck waiting for a restart — use the admin UI's "Rebuild Search Index" button while the app is running, or `synap search reindex` while it's stopped (see On-Demand Reindex above).


