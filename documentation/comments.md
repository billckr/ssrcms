---
title: Comments & Replies
group: feature
updated_by: claude
last_updated: 2026-07-22
---
# Comments & Replies

> Last updated: 2026-07-22 | Updated by: claude

## Overview

Registered account users (subscribers or above) can submit comments on published posts that have `comments_enabled = true`. Comments support one level of threading via `parent_id` (replies to top-level comments only). Comments can be soft-deleted by their author or hard-deleted by admins with the `can_manage_content` capability.

## How It Works

### Submission (`core/src/handlers/comment.rs`, `POST /{slug}/comment`)

Steps performed by `submit`:
1. Requires an active account session (`SESSION_ACCOUNT_USER_ID_KEY`); redirects to `/login?redirect={post_url}` if absent.
2. Rejects if the "I'm human" checkbox (`human_check`) was not ticked.
3. Validates `body` is non-empty and trimmed length ≤ 400 chars.
4. Fetches the post and confirms `comments_enabled` is true.
5. Rate-limits to 2 comments (top-level or reply) per user per 10 minutes, checked via a live COUNT query against `comments.created_at`.
6. Records the submitter's IP (`X-Real-IP` → `X-Forwarded-For` → socket address) into `ip_address` (migration 0031).
7. Inserts the comment and redirects to `/{slug}#comments`.

### Data Model (`core/src/models/comment.rs`)

`Comment` columns: `id`, `post_id`, `site_id`, `author_id`, `parent_id`, `body`, `ip_address`, `created_at`, `updated_at`, `deleted_at` (soft-delete, migration 0030). Body length is capped at the database level too (migration 0029).

`list_for_post()` builds a two-level (top-level + replies) tree and applies soft-delete display rules:
- A deleted **reply** is excluded entirely.
- A deleted **top-level comment with remaining (non-deleted) replies** is kept with body blanked and `is_deleted = true` (template renders "[deleted]").
- A deleted **top-level comment with no replies** is excluded entirely.

Results are paginated (10 per page, `CommentPage`).

### Account Comment History

`list_for_user` / `count_for_user` support the subscriber-facing "My Comments" page with search across comment body and post title (stop words stripped via `search_terms`), always excluding soft-deleted comments.

### Moderation

`POST /admin/comments/{id}/delete` — hard-delete, requires `admin.caps.can_manage_content` (redirects to `/admin` otherwise).
`POST /account/comments/{id}/delete` — soft-delete, only succeeds if the comment belongs to the requesting `author_id` and isn't already deleted (`soft_delete` uses an atomic `UPDATE ... WHERE id = $1 AND author_id = $2 AND deleted_at IS NULL`).

## Routes / Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | /{slug}/comment | `comment::submit` | Submit comment (requires account session) |
| GET | /account/my-comments | `account::my_comments` | Subscriber's own comment history (searchable) |
| POST | /account/comments/{id}/delete | `account::delete_comment` | Author self-delete (soft) |
| POST | /admin/comments/{id}/delete | `admin_comments::delete` | Admin hard delete |

## Database Schema

`comments` table (migration 0028, extended by 0029–0031): `id`, `post_id`, `site_id`, `author_id`, `parent_id` (self-referential, one level), `body` (length-limited), `ip_address`, `created_at`, `updated_at`, `deleted_at`.

## Security Notes

- Submission requires an active account session — no anonymous comments.
- Simple honeypot-style human check (`human_check` checkbox) rather than a CAPTCHA.
- Rate limit: 2 comments per user per 10-minute rolling window (checked against the DB directly, not the site-wide IP allow/block lists, which live in `core/src/middleware/ip_allowlist.rs` / `ip_denylist.rs` and apply at the request level, not comment-specific).
- Admin hard-delete requires the `can_manage_content` capability; author self-delete is scoped to their own `author_id` via the SQL `WHERE` clause, not just an application-level check.
