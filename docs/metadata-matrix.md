# Metadata Matrix

Date: 2026-04-14

Purpose: define the canonical metadata inventory for `noxa` now, even where implementation is deferred. This keeps schema design stable while allowing implementation to land in priority order.

## Rules

- Map the full candidate schema now.
- Implement in priority order, not all at once.
- Only index fields with a real near-term query path.
- Prefer nullable additions over renames/removals.
- Keep source-specific enrichment behind actual source support.

## Priority Tiers

- `Now`: already implemented or should be treated as current foundation
- `Next`: implement after the current foundation, highest-value follow-up
- `Later`: useful, but baseline extraction already exists or the source is not first-order yet
- `Deferred`: only implement when a dedicated first-class source/ingestor exists

## Canonical Field Matrix

| Field | Type | Source families | Status | Index | Priority | Notes |
|---|---|---|---|---|---|---|
| `url` | string | all | Now | keyword | Now | Canonical document identifier |
| `domain` | string | all | Now | keyword | Now | Use `local` for local files |
| `title` | string? | web, file, email, rss, youtube, reddit | Now | no | Now | Search result display field |
| `author` | string? | web, file, email, youtube, reddit, paperless | Now | no | Now | Generic human/source author |
| `published_date` | string? | web, email, rss, youtube, reddit | Now | no | Now | Generic publish/send date |
| `language` | string? | web, file, rss, email | Now | keyword | Now | Current payload/search-result field |
| `source_type` | string? | all | Now | keyword | Now | Current values include `web`, `file`; `rss` should be added when RSS provenance lands |
| `content_hash` | string? | all | Now | no | Now | Dedup/version key |
| `technologies` | string[] | web, file, bookmarks, memos, snippets, paperless | Now | no | Now | Current cross-source taxonomy bucket |
| `is_truncated` | bool? | all | Now | no | Now | Signals chunk-cap truncation |
| `file_path` | string? | file | Now | no | Now | Absolute path for local sources |
| `last_modified` | string? | file | Now | no | Now | Current generic file mtime |
| `git_branch` | string? | file | Now | no | Now | Current minimal git provenance |
| `seed_url` | string? | web | Now | no | Now | Crawl provenance |
| `search_query` | string? | web, rss | Now | no | Now | Search provenance |
| `crawl_depth` | u32? | web | Now | no | Now | Crawl provenance |
| `fetched_at` | string? | web | Now | no | Now | Present in `Metadata`, not yet a core rag payload field |
| `external_id` | string? | rss, email, mcp, github, reddit, youtube | Partial | keyword | Next | Present in rag types; use for stable dedup across URL variants |
| `platform_url` | string? | mcp, github | Partial | no | Deferred | Keep for UI/native source URLs |
| `feed_url` | string? | rss | Planned | keyword | Next | Feed-level provenance |
| `feed_name` | string? | rss | Planned | no | Next | Human feed title |
| `feed_item_guid` | string? | rss | Planned | keyword | Next | Stable dedup key for RSS items |
| `email_from` | string? | email | Planned | keyword | Next | Sender |
| `email_to` | string[] | email | Planned | no | Next | Recipient list |
| `email_subject` | string? | email | Planned | no | Next | Often same as display title |
| `email_date` | string? | email | Planned | no | Next | Can map to `published_date`, but keep raw provenance too |
| `email_message_id` | string? | email | Planned | keyword | Next | Stable dedup key |
| `email_thread_id` | string? | email | Planned | keyword | Next | Thread grouping |
| `email_has_attachments` | bool? | email | Planned | no | Next | Useful filter later |
| `mcp_server` | string? | mcp | Planned | keyword | Deferred | Only when MCP ingestion becomes first-class |
| `mcp_tool` | string? | mcp | Planned | keyword | Deferred | Same |
| `linkding_tags` | string[] | mcp/linkding | Planned | no | Deferred | Generic `technologies` covers most current need |
| `memos_visibility` | string? | mcp/memos | Planned | keyword | Deferred | Only if memos source lands |
| `snippet_language` | string? | mcp/bytestash | Planned | keyword | Deferred | Might later merge with generic code-language field |
| `paperless_correspondent` | string? | mcp/paperless | Planned | keyword | Deferred | Only if paperless lands |
| `paperless_document_type` | string? | mcp/paperless | Planned | keyword | Deferred | Same |
| `youtube_video_id` | string? | youtube | Planned | keyword | Later | Baseline YouTube extraction already exists |
| `youtube_channel_name` | string? | youtube | Planned | keyword | Later | Same |
| `youtube_published_at` | string? | youtube | Planned | no | Later | Same |
| `youtube_duration_s` | u32? | youtube | Planned | no | Later | Same |
| `youtube_start_s` | u32? | youtube/transcript chunks | Planned | integer | Later | Only when chunking real transcripts/captions |
| `youtube_end_s` | u32? | youtube/transcript chunks | Planned | no | Later | Same |
| `youtube_transcript_type` | string? | youtube | Planned | keyword | Later | Only when transcript ingestion is first-class |
| `reddit_subreddit` | string? | reddit | Planned | keyword | Later | Baseline Reddit extraction already exists |
| `reddit_post_id` | string? | reddit | Planned | keyword | Later | Stable ID |
| `reddit_comment_id` | string? | reddit | Planned | keyword | Later | Stable ID for comment chunks |
| `reddit_score` | i64? | reddit | Planned | integer | Later | Ranking/filter signal |
| `reddit_num_comments` | u32? | reddit | Planned | no | Later | Same |
| `github_repo` | string? | github | Planned | keyword | Deferred | Only when GitHub becomes a dedicated source |
| `github_content_type` | string? | github | Planned | keyword | Deferred | issue/pr/code/release/etc. |
| `github_number` | u64? | github | Planned | no | Deferred | PR/issue number |
| `github_labels` | string[] | github | Planned | keyword | Deferred | Useful once GitHub source lands |
| `ai_tool` | string? | ai_session | Planned | keyword | Deferred | Only for a dedicated AI session ingestor |
| `ai_model` | string? | ai_session | Planned | keyword | Deferred | Same |
| `session_id` | string? | ai_session | Planned | keyword | Deferred | Same |

## Recommended Index Set

### Keep now

- `url`
- `domain`
- `source_type`
- `language`

### Add next

- `external_id`
- `feed_url`
- `feed_item_guid`
- `email_from`
- `email_message_id`
- `email_thread_id`

### Add later only with source support

- YouTube-specific indexes
- Reddit-specific indexes
- MCP/platform-specific indexes
- GitHub/AI-session-specific indexes

## Implementation Order

1. RSS/email provenance metadata
2. Dedup/index support for `external_id`, RSS GUIDs, and email message/thread IDs
3. YouTube chunk/deep-link metadata if transcript ingestion becomes first-class
4. Reddit enrichment beyond the current JSON fallback
5. GitHub and AI-session metadata only when those become dedicated ingestors

## Non-Goals For Now

- Implement every source-specific field immediately
- Add indexes for speculative fields with no current query path
- Rework the current minimal schema foundation that already landed
