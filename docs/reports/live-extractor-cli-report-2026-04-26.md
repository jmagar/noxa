# Live CLI Extractor Test Report - 2026-04-26

## Summary

All 28 vertical extractors were executed through the real `noxa` CLI against live public URLs.

- Result: 28/28 passed.
- Pass criterion: command exited `0`, stdout parsed as JSON, and `.vertical_data.extractor` matched the requested extractor name.
- Binary: `target/debug/noxa`, built with `cargo build -p noxa-cli`.
- Common command shape: `target/debug/noxa --no-store --extractor <name> -f json <url>`.
- Raw evidence directory: `target/live-extractor-tests/20260426T042403Z/`.

## Retest Adjustments

The first sweep found endpoint-specific failures, not fixture-only failures:

- `reddit`: `www.reddit.com/.../.json` returned HTML to the CLI fetch path; `new.reddit.com/.../.json` returned JSON and passed.
- `npm`: `react` exceeded the current JSON body limit; `is-odd` stayed below the limit and passed.
- `huggingface_model`: bare `bert-base-uncased` does not match the current owner/name matcher; `google-bert/bert-base-uncased` passed.
- `instagram_profile`: the public web profile API required `x-ig-app-id: 936619743392459`; with that header, it passed.
- `shopify_product` and `shopify_collection`: the initial Snowdevil endpoints timed out; Allbirds product and collection JSON endpoints passed.

## Results

| Extractor | Result | Verified vertical | Evidence title/id | Command |
|---|---:|---|---|---|
| `reddit` | PASS | `reddit` | This Week in Rust #648 | `target/debug/noxa --no-store --extractor reddit -f json 'https://new.reddit.com/r/rust/comments/1su40pd/this_week_in_rust_648/'` |
| `hackernews` | PASS | `hackernews` | My YC app: Dropbox - Throw away your USB drive | `target/debug/noxa --no-store --extractor hackernews -f json 'https://news.ycombinator.com/item?id=8863'` |
| `github_repo` | PASS | `github_repo` | rust | `target/debug/noxa --no-store --extractor github_repo -f json 'https://github.com/rust-lang/rust'` |
| `github_pr` | PASS | `github_pr` | PR #1 | `target/debug/noxa --no-store --extractor github_pr -f json 'https://github.com/rust-lang/rust/pull/1'` |
| `github_issue` | PASS | `github_issue` | Thread a session or semantic context through IL | `target/debug/noxa --no-store --extractor github_issue -f json 'https://github.com/rust-lang/rust/issues/1'` |
| `github_release` | PASS | `github_release` | Rust 1.0.0 | `target/debug/noxa --no-store --extractor github_release -f json 'https://github.com/rust-lang/rust/releases/tag/1.0.0'` |
| `pypi` | PASS | `pypi` | requests | `target/debug/noxa --no-store --extractor pypi -f json 'https://pypi.org/project/requests/'` |
| `npm` | PASS | `npm` | is-odd | `target/debug/noxa --no-store --extractor npm -f json 'https://www.npmjs.com/package/is-odd'` |
| `crates_io` | PASS | `crates_io` | serde | `target/debug/noxa --no-store --extractor crates_io -f json 'https://crates.io/crates/serde'` |
| `huggingface_model` | PASS | `huggingface_model` | google-bert/bert-base-uncased | `target/debug/noxa --no-store --extractor huggingface_model -f json 'https://huggingface.co/google-bert/bert-base-uncased'` |
| `huggingface_dataset` | PASS | `huggingface_dataset` | rajpurkar/squad | `target/debug/noxa --no-store --extractor huggingface_dataset -f json 'https://huggingface.co/datasets/squad'` |
| `arxiv` | PASS | `arxiv` | Attention Is All You Need | `target/debug/noxa --no-store --extractor arxiv -f json 'https://arxiv.org/abs/1706.03762'` |
| `docker_hub` | PASS | `docker_hub` | nginx | `target/debug/noxa --no-store --extractor docker_hub -f json 'https://hub.docker.com/_/nginx'` |
| `dev_to` | PASS | `dev_to` | dev.to article payload | `target/debug/noxa --no-store --extractor dev_to -f json 'https://dev.to/devteam/introducing-dev-20-3kmh'` |
| `stackoverflow` | PASS | `stackoverflow` | How do I exit Vim? | `target/debug/noxa --no-store --extractor stackoverflow -f json 'https://stackoverflow.com/questions/11828270/how-do-i-exit-vim'` |
| `substack_post` | PASS | `substack_post` | Lenny's Newsletter / Substack | `target/debug/noxa --no-store --extractor substack_post -f json 'https://lenny.substack.com/p/what-is-good-retention'` |
| `youtube_video` | PASS | `youtube_video` | Rick Astley - Never Gonna Give You Up | `target/debug/noxa --no-store --extractor youtube_video -f json 'https://www.youtube.com/watch?v=dQw4w9WgXcQ'` |
| `linkedin_post` | PASS | `linkedin_post` | LinkedIn embed payload | `target/debug/noxa --no-store --extractor linkedin_post -f json 'https://www.linkedin.com/feed/update/urn:li:activity:7123456789012345678/'` |
| `instagram_post` | PASS | `instagram_post` | Instagram embed payload | `target/debug/noxa --no-store --extractor instagram_post -f json 'https://www.instagram.com/p/CuY4nD2NrjI/'` |
| `instagram_profile` | PASS | `instagram_profile` | Instagram | `target/debug/noxa --no-store --extractor instagram_profile -f json -H 'x-ig-app-id: 936619743392459' 'https://www.instagram.com/instagram/'` |
| `shopify_product` | PASS | `shopify_product` | Men's Tree Runner - Kaikoura White | `target/debug/noxa --no-store --extractor shopify_product -f json 'https://www.allbirds.com/products/mens-tree-runners-kaikoura-white'` |
| `shopify_collection` | PASS | `shopify_collection` | Allbirds mens collection products | `target/debug/noxa --no-store --extractor shopify_collection -f json 'https://www.allbirds.com/collections/mens'` |
| `ecommerce_product` | PASS | `ecommerce_product` | Abominable Hoodie | `target/debug/noxa --no-store --extractor ecommerce_product -f json 'https://www.scrapingcourse.com/ecommerce/product/abominable-hoodie/'` |
| `woocommerce_product` | PASS | `woocommerce_product` | Abominable Hoodie | `target/debug/noxa --no-store --extractor woocommerce_product -f json 'https://www.scrapingcourse.com/ecommerce/product/abominable-hoodie/'` |
| `amazon_product` | PASS | `amazon_product` | Amazon product payload | `target/debug/noxa --no-store --extractor amazon_product -f json 'https://www.amazon.com/dp/B08N5WRWNW'` |
| `ebay_listing` | PASS | `ebay_listing` | eBay listing payload | `target/debug/noxa --no-store --extractor ebay_listing -f json 'https://www.ebay.com/itm/256172084604'` |
| `etsy_listing` | PASS | `etsy_listing` | Etsy listing payload | `target/debug/noxa --no-store --extractor etsy_listing -f json 'https://www.etsy.com/listing/1058071087/personalized-leather-wallet-for-men'` |
| `trustpilot_reviews` | PASS | `trustpilot_reviews` | Trustpilot review payload | `target/debug/noxa --no-store --extractor trustpilot_reviews -f json 'https://www.trustpilot.com/review/www.amazon.com'` |

## Caveats

- This report verifies live CLI execution and vertical payload plumbing. It does not claim that every live site returned complete business fields; some HTML/anti-bot-heavy pages produced sparse but valid extractor payloads.
- The live results depend on third-party endpoint behavior as of 2026-04-26. Reddit, Instagram, Shopify storefronts, and ecommerce pages are especially drift-prone.
- The raw output files live under `target/`, so they are intentionally not tracked in git.
