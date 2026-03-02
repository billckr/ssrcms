//! Tantivy full-text search index.
//!
//! Schema:
//!   id          — TEXT STORED          (UUID string, used to fetch full post from DB)
//!   site_id     — TEXT STORED          (UUID string, used to filter results by site)
//!   title       — TEXT indexed+stored  (searched, returned for scoring boost)
//!   content     — TEXT indexed only    (searched, not stored — saves space)
//!   slug        — TEXT STORED          (for URL building without a DB round-trip)
//!   post_type   — TEXT STORED+fast     (filter: "post" vs "page")

use std::path::Path;
use std::sync::{Arc, RwLock};

use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};

use crate::errors::{AppError, Result};

/// Fields available in the Tantivy schema.
#[derive(Clone)]
pub struct SearchSchema {
    pub schema: Schema,
    pub id: Field,
    pub site_id: Field,
    pub title: Field,
    pub content: Field,
    pub slug: Field,
    pub post_type: Field,
}

impl SearchSchema {
    pub fn build() -> Self {
        let mut builder = Schema::builder();

        let id = builder.add_text_field("id", STRING | STORED);
        let site_id = builder.add_text_field("site_id", STORED);
        let title = builder.add_text_field("title", TEXT | STORED);
        let content = builder.add_text_field("content", TEXT);
        let slug = builder.add_text_field("slug", STORED);
        let post_type = builder.add_text_field("post_type", STRING | STORED);

        SearchSchema {
            schema: builder.build(),
            id,
            site_id,
            title,
            content,
            slug,
            post_type,
        }
    }
}

/// A single search result returned by `SearchIndex::search()`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub post_type: String,
    pub score: f32,
}

/// Thread-safe wrapper around a Tantivy index.
/// Clone is cheap — the inner index and reader are Arc-wrapped.
#[derive(Clone)]
pub struct SearchIndex {
    pub fields: SearchSchema,
    index: Index,
    reader: IndexReader,
    /// Writer is behind a Mutex so concurrent writes are serialized.
    writer: Arc<RwLock<IndexWriter>>,
}

impl SearchIndex {
    /// Open an existing index at `path`, or create a new one.
    pub fn open_or_create(path: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(path)?;

        let fields = SearchSchema::build();

        let index = match Index::open_in_dir(path) {
            Ok(existing) if existing.schema() == fields.schema => existing,
            Ok(_) => {
                // Schema mismatch — wipe and recreate.
                tracing::warn!("search index schema mismatch — recreating index");
                std::fs::remove_dir_all(path)?;
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, fields.schema.clone())?
            }
            Err(_) => Index::create_in_dir(path, fields.schema.clone())?,
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        // 50 MB write buffer
        let writer = index.writer(50_000_000)?;

        Ok(SearchIndex {
            fields,
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
        })
    }

    /// Execute a full-text search and return up to `limit` results.
    /// If `site_id` is Some, only results belonging to that site are returned.
    pub fn search(&self, query_str: &str, site_id: Option<&str>, limit: usize) -> Result<Vec<SearchResult>> {
        if query_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.title, self.fields.content]);

        let query = query_parser
            .parse_query(query_str)
            .unwrap_or_else(|_| {
                // Special characters in the query (e.g. +, -, :) can cause parse errors.
                // Fall back to a literal search on the escaped string so we return empty
                // results rather than a 500.
                query_parser
                    .parse_query_lenient(query_str)
                    .0
            });

        // Fetch more than `limit` to allow for site_id post-filtering.
        let fetch_limit = if site_id.is_some() { limit * 4 + 20 } else { limit };

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(fetch_limit))
            .map_err(|e| AppError::Internal(format!("search error: {e}")))?;

        let mut results = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| AppError::Internal(format!("doc fetch error: {e}")))?;

            let id = doc
                .get_first(self.fields.id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let doc_site_id = doc
                .get_first(self.fields.site_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = doc
                .get_first(self.fields.title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let slug = doc
                .get_first(self.fields.slug)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let post_type = doc
                .get_first(self.fields.post_type)
                .and_then(|v| v.as_str())
                .unwrap_or("post")
                .to_string();

            // Post-filter by site_id if provided.
            if let Some(sid) = site_id {
                if doc_site_id != sid {
                    continue;
                }
            }

            results.push(SearchResult { id, title, slug, post_type, score });
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    /// Replace the entire index with a new set of documents in a single commit.
    /// Use this for bulk startup rebuilds — vastly faster than calling upsert()
    /// per document (which commits after every write, causing N disk flushes).
    pub fn rebuild_all(&self, docs: &[(String, String, String, String, String, String)]) -> anyhow::Result<()> {
        let mut writer = self.writer.write().unwrap();
        writer.delete_all_documents()?;
        for (id, site_id, title, content, slug, post_type) in docs {
            let mut doc = TantivyDocument::default();
            doc.add_text(self.fields.id, id);
            doc.add_text(self.fields.site_id, site_id);
            doc.add_text(self.fields.title, title);
            doc.add_text(self.fields.content, content);
            doc.add_text(self.fields.slug, slug);
            doc.add_text(self.fields.post_type, post_type);
            writer.add_document(doc)?;
        }
        writer.commit()?;
        Ok(())
    }

    /// Add or update a document. Tantivy doesn't have native upsert — we delete
    /// by id term then add the new document, then commit.
    pub fn upsert(&self, id: &str, site_id: &str, title: &str, content: &str, slug: &str, post_type: &str) -> anyhow::Result<()> {
        let mut writer = self.writer.write().unwrap();

        // Delete any existing document with this id.
        let id_term = Term::from_field_text(self.fields.id, id);
        writer.delete_term(id_term);

        // Add the new document.
        let mut doc = TantivyDocument::default();
        doc.add_text(self.fields.id, id);
        doc.add_text(self.fields.site_id, site_id);
        doc.add_text(self.fields.title, title);
        doc.add_text(self.fields.content, content);
        doc.add_text(self.fields.slug, slug);
        doc.add_text(self.fields.post_type, post_type);
        writer.add_document(doc)?;

        writer.commit()?;
        Ok(())
    }

    /// Remove a document by post UUID string.
    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let mut writer = self.writer.write().unwrap();
        let id_term = Term::from_field_text(self.fields.id, id);
        writer.delete_term(id_term);
        writer.commit()?;
        Ok(())
    }
}
