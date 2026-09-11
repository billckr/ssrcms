---
title: Poll Designer
group: feature
updated_by: codex
last_updated: 2026-09-10
---
# Poll Designer

> Last updated: 2026-09-10 | Updated by: codex

## Overview

Poll Designer creates reusable, single-question polls that can be embedded in classic posts and
pages. A poll owns its question, option labels and stable option keys, vote-button/success text,
and duplicate-vote protection. Votes and results remain attached to that one reusable poll even
when it appears in multiple posts or languages.

## Creating and Embedding a Poll

Open **Designer → Polls**, create a poll with at least two options, and save it. The slug is created
once and remains stable. In a post/page editor, use the poll toolbar picker to insert an inert
`<ss-poll>` marker. Public rendering expands that marker into the current poll definition, so a
poll can be edited once and reused across content.

The source post/page controls where the marker appears. Adding, removing, or moving only a poll
marker on an already translated post does not make its translated prose stale or trigger an AI
request. At render time SynapCMS mechanically reconciles current source markers into the stored
localized body.

## Translation Workflow

When installation-wide AI Translation is enabled, a saved poll exposes a **Translations** section.
Choose an enabled site language and verified provider to translate the question, option labels,
button text, success text, and total-vote label. Option keys remain unchanged, so every language
continues to share the same vote rows, totals, exports, and analytics.

There are two equivalent ways to create or refresh a poll translation:

- Translate the reusable poll directly from Poll Designer. This is useful when it is embedded in
  several posts/pages or you want to manage it independently.
- In a post/page that already has a translation for the locale, use **Translate embedded items
  only** (layers). It processes only embedded forms/polls that are missing or **Source changed**.

Neither path translates post/page prose. Editing the poll makes its localized payloads stale, but
does not make containing post translations stale. Until refreshed, the stored localized payload
can remain visible; if no valid locale payload exists, visitors see the source-language poll.

## Voting and Localized Results

The public form posts the stable option key to `POST /poll/{slug}`. Duplicate-vote protection uses
a signed long-lived browser cookie and, by default, the visitor IP address. After voting, the
embedded script requests `GET /poll/{slug}/results`; localized pages include the locale so result
option labels and the total-vote label use the same poll translation. Vote counts are shared across
all languages.

## Routes

| Method | Path | Purpose |
|---|---|---|
| GET/POST | `/admin/designer/polls` | List or create polls |
| GET | `/admin/designer/polls/new` | New poll editor |
| GET/POST | `/admin/designer/polls/{id}` | Edit a poll |
| POST | `/admin/designer/polls/{id}/translate` | Translate or refresh one locale |
| POST | `/admin/designer/polls/{id}/translations/{locale}/delete` | Delete one localized payload |
| GET | `/admin/designer/polls/{id}/results` | View poll results |
| GET | `/admin/designer/polls/{id}/results/export` | Export results |
| POST | `/admin/designer/polls/{id}/results/reset` | Reset results |
| POST | `/poll/{slug}` | Record a public vote |
| GET | `/poll/{slug}/results` | Return public result totals and localized labels |

## Data and Safety

- `polls` stores the reusable definition; `poll_votes` stores votes by stable option key.
- `poll_translations` stores one visitor-facing JSON payload per poll and locale, with the source
  update timestamp used for freshness status.
- Model responses must preserve every stable option key and the `{count}` result placeholder.
  Invalid identity, cardinality, or placeholders reject the complete response before persistence.
- Deleting a poll cascades to its votes and localized payloads. Deleting one translation never
  affects votes or the source poll.

See **AI Post Translation** for provider setup, global enable/disable behavior, logging, and the
complete post/page translation flow.
