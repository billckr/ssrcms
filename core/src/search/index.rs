//! Tantivy full-text search index.
//!
//! Schema:
//!   id          — TEXT STORED          (UUID string, used to fetch full post from DB)
//!   site_id     — TEXT STORED          (UUID string, used to filter results by site)
//!   title       — TEXT indexed+stored  (searched, returned for scoring boost)
//!   content     — TEXT indexed only    (searched, not stored — saves space)
//!   slug        — TEXT STORED          (for URL building without a DB round-trip)
//!   post_type   — TEXT STORED+fast     (filter: "post" vs "page")
//!
//! Tokenizer: "en_stop" — SimpleTokenizer → LowerCaser → StopWordFilter → Stemmer(English)
//! Stop words are stripped at both index time and query time, so searching common
//! words like "and", "the", "is" returns no results rather than matching everything.

use std::path::Path;
use std::sync::{Arc, RwLock};

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, PhrasePrefixQuery, Query, QueryParser};
use tantivy::schema::*;
use tantivy::tokenizer::{Language, LowerCaser, SimpleTokenizer, Stemmer, StopWordFilter, TextAnalyzer};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};

/// Name used to register the custom analyzer with the index.
const TOKENIZER_NAME: &str = "en_stop";

/// Common English stop words stripped before indexing and querying.
/// Searching any of these terms alone returns zero results.
static EN_STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "up", "about", "into", "through", "is",
    "was", "are", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might",
    "shall", "can", "i", "me", "my", "we", "our", "you", "your", "he",
    "him", "his", "she", "her", "it", "its", "they", "them", "their",
    "this", "that", "these", "those", "what", "which", "who", "whom",
    "not", "no", "so", "if", "as", "than", "too", "very", "just", "also",
    "more", "most", "other", "some", "such", "only", "own", "same",
];

/// Build the custom English text analyzer:
/// tokenise → lowercase → strip stop words → stem.
fn build_analyzer() -> TextAnalyzer {
    let stops: Vec<String> = EN_STOP_WORDS.iter().map(|s| s.to_string()).collect();
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .filter(StopWordFilter::remove(stops))
        .filter(Stemmer::new(Language::English))
        .build()
}

use crate::errors::{AppError, Result};

/// Extracts the last whitespace-separated word of a query, lowercased with
/// punctuation stripped, for the as-you-type prefix match. `None` for an
/// empty trailing word or one shorter than 2 characters — a 1-character
/// prefix would expand against too much of the term dictionary to be a
/// useful match while someone's still typing the first keystroke.
fn last_token_prefix(query_str: &str) -> Option<String> {
    let tok: String = query_str
        .split_whitespace()
        .next_back()?
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if tok.chars().count() >= 2 { Some(tok) } else { None }
}

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

        // Indexing options for searchable fields — uses the "en_stop" custom tokenizer
        // (stop words + stemming) registered on the index at startup.
        let indexed = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TOKENIZER_NAME)
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            );

        let id = builder.add_text_field("id", STRING | STORED);
        let site_id = builder.add_text_field("site_id", STORED);
        let title = builder.add_text_field("title", indexed.clone() | STORED);
        let content = builder.add_text_field("content", indexed);
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
                // Schema mismatch (e.g. tokenizer changed) — wipe and recreate.
                tracing::warn!("search index schema mismatch — recreating index");
                std::fs::remove_dir_all(path)?;
                std::fs::create_dir_all(path)?;
                Index::create_in_dir(path, fields.schema.clone())?
            }
            Err(_) => Index::create_in_dir(path, fields.schema.clone())?,
        };

        // Register the custom analyzer so both indexing and querying use it.
        index.tokenizers().register(TOKENIZER_NAME, build_analyzer());

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

        let base_query: Box<dyn Query> = query_parser
            .parse_query(query_str)
            .unwrap_or_else(|_| {
                // Special characters in the query (e.g. +, -, :) can cause parse errors.
                // Fall back to a literal search on the escaped string so we return empty
                // results rather than a 500.
                query_parser
                    .parse_query_lenient(query_str)
                    .0
            });

        // As-you-type support: OR in a prefix match on the in-progress last word
        // (e.g. "advanc" matches "Advanced" before the whole word — or its
        // stem — has been typed), on top of the normal stemmed match above.
        // A single-term `PhrasePrefixQuery` degrades to a cheap term-dictionary
        // range scan (bounded by its default 50-expansion cap) rather than a
        // full-index scan, so this adds negligible cost per query and needs no
        // schema change or reindex. It still matches correctly against the
        // stemmed dictionary: English stemming only strips suffixes, so a
        // lowercased raw prefix remains a valid prefix of the stemmed term it
        // will eventually complete into.
        let query: Box<dyn Query> = match last_token_prefix(query_str) {
            Some(prefix) => Box::new(BooleanQuery::new(vec![
                (Occur::Should, base_query),
                (Occur::Should, Box::new(PhrasePrefixQuery::new(vec![
                    Term::from_field_text(self.fields.title, &prefix),
                ]))),
                (Occur::Should, Box::new(PhrasePrefixQuery::new(vec![
                    Term::from_field_text(self.fields.content, &prefix),
                ]))),
            ])),
            None => base_query,
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    fn index_with_doc(title: &str) -> SearchIndex {
        let path = std::env::temp_dir().join(format!("synaptic-search-test-{}", uuid::Uuid::new_v4()));
        let index = SearchIndex::open_or_create(&path).unwrap();
        index.upsert("1", "site-a", title, "", "test-post", "post").unwrap();
        // ReloadPolicy::OnCommitWithDelay reloads the reader asynchronously
        // shortly after a commit — force it synchronously here so the doc
        // is visible to the very next search() call in the test.
        index.reader.reload().unwrap();
        index
    }

    #[test]
    fn prefix_of_a_stemmed_word_matches_before_the_word_is_complete() {
        let index = index_with_doc("Advanced Comparison");
        for q in ["ad", "adv", "advan", "advanc", "advance", "advanced"] {
            let results = index.search(q, None, 10).unwrap();
            assert!(!results.is_empty(), "expected a match for query {q:?}");
        }
    }

    #[test]
    fn single_char_query_does_not_prefix_match() {
        // last_token_prefix requires >= 2 chars — a 1-char query still runs
        // (via the normal stemmed parse), it just gets no prefix-match boost.
        let index = index_with_doc("Advanced Comparison");
        let results = index.search("a", None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn unrelated_prefix_does_not_match() {
        let index = index_with_doc("Advanced Comparison");
        let results = index.search("xyz", None, 10).unwrap();
        assert!(results.is_empty());
    }
}
