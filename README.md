# twitter-dl

## Project Overview
A Twitter/X video downloader with a plain HTML/CSS/JS frontend and a Rust (Axum) backend.
Users paste a tweet URL and can download the video as MP4, captions as SRT, or audio only.

## Stack
- **Frontend**: Vanilla HTML, CSS, JS — no framework, no bundler
- **Backend**: Rust with Axum, reqwest, tokio, serde
- **No build step for frontend** — just open index.html or serve statically

## Directory Structure
```
twitter-dl/
├── frontend/
│   ├── css/
│   │   ├── styles.css    # Design tokens, layout, dark theme
│   │   └── tweet.css     # Tweet card, captions mode styles
│   ├── js/
│   │   ├── api.js        # fetch() calls to Rust backend
│   │   ├── app.js        # UI state, event handlers, validation
│   │   └── downloader.js # blob download helpers
│   └── index.html
├── src/
│   ├── main.rs           # Axum server entry, router, rate limiter
│   ├── error.rs          # AppError type
│   ├── models/
│   │   ├── mod.rs
│   │   ├── tweet.rs      # Syndication API response structs
│   │   └── video_variant.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── audio.rs      # POST /api/audio
│   │   ├── captions.rs   # POST /api/captions
│   │   ├── download.rs   # POST /api/download
│   │   ├── info.rs       # POST /api/info
│   │   └── preview.rs    # GET /api/preview (video proxy)
│   └── services/
│       ├── mod.rs
│       ├── captions.rs   # VTT→SRT conversion
│       ├── download.rs   # MP4/SRT merging helpers
│       ├── twitter.rs    # Guest token, syndication API calls
│       └── video.rs      # HLS segment merging, ffmpeg remux
├── .env                  # HOST, PORT
├── .env.example
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── README.md
└── AGENTS.md
```

## API Contract
The frontend (js/api.js) expects the backend running at `http://localhost:3000/api`.

| Method | Endpoint        | Body                        | Returns             |
|--------|-----------------|-----------------------------|---------------------|
| POST   | /api/info       | `{ url }`                   | TweetInfo JSON      |
| POST   | /api/download   | `{ url, quality, include_quote, include_reply }` | MP4 blob stream |
| POST   | /api/captions   | `{ url, include_quote, include_reply }`          | SRT blob        |
| POST   | /api/audio      | `{ url, quality, include_quote, include_reply }` | M4A blob        |

### TweetInfo response shape
```json
{
  "author": "username",
  "created_at": "May 7, 2025",
  "text": "tweet content...",
  "quoted_tweet": null | { "author": "...", "text": "..." },
  "in_reply_to": null | { "author": "...", "text": "..." },
  "variants": [
    { "label": "1080p", "url": "...", "bitrate": 2176000 },
    { "label": "720p",  "url": "...", "bitrate": 832000  }
  ],
  "avatar_url": null | "https://pbs.twimg.com/profile_images/..._normal.jpg",
  "likes": null | 1234
}
```

## Running the Project

### Backend
```bash
cp .env.example .env       # (optional — defaults are 127.0.0.1:3000)
cargo run                  # starts on localhost:3000
```

### Frontend
```bash
# No build step needed — just serve statically
npx serve frontend         # or open frontend/index.html directly in browser
```

## Key Conventions

### Rust
- Use `AppError` (defined in `error.rs`) for all route error returns — implement `IntoResponse`
- Services return `Result<T, AppError>`, routes just `?`-propagate
- Twitter guest token logic lives exclusively in `services/twitter.rs`
- HLS segment merging lives in `services/video.rs` — shell out to `ffmpeg` for remuxing
- All external HTTP calls go through a shared `reqwest::Client` passed via Axum state

### Frontend (JS)
- `app.js` owns all UI state — do not manipulate DOM from `api.js` or `downloader.js`
- `api.js` is pure fetch functions only — no DOM, no state
- `downloader.js` is pure blob/download helpers — no DOM, no state
- URL validation regex: `/^https?:\/\/(twitter\.com|x\.com)\/.+\/status\/\d+/`
- Quality selection is stored in the `selectedQuality` variable in `app.js`

### CSS
- All design tokens are CSS variables at `:root` in `styles.css` — never hardcode colors
- Font: JetBrains Mono for monospace elements, Geist for body text
- Color palette is Zed-inspired dark theme — see `:root` variables

## Environment Variables
```
HOST=127.0.0.1          # optional, default 127.0.0.1
PORT=3000               # optional, default 3000
CORS_ORIGIN             # optional, restricts CORS in production (omit for dev = AllowOrigin(Any))
```

## Deployment

### Backend Hosting

The Rust backend runs as a single binary — no Node.js runtime needed. Good options:

| Platform | Pros | Cons |
|----------|------|------|
| **[Railway](https://railway.app)** | Rust builder built-in, free tier, git push deploy, env var management, custom domains | No SSH access |
| **[Fly.io](https://fly.io)** | Global edge regions, free tier, Dockerfile or Rust builder, WireGuard tunnel | Slightly steeper config |
| **[Shuttle.rs](https://shuttle.rs)** | Made for Rust, one-command deploy (`cargo shuttle deploy`), free tier, integrates with Axum natively | Less portable, vendor lock-in |

**All three** give you a public URL, handle TLS/SSL, and let you set env vars (like `HOST`, `PORT`). For a personal tool, Railway is the sweet spot — minimal config, Rust detected automatically.

### Database (for download counters, etc.)

| Platform | Pros | Cons |
|----------|------|------|
| **[Supabase](https://supabase.com)** (PostgreSQL) | Free tier, great Rust support via `sqlx`, hosted, dashboard UI | Overkill if you only need a counter |
| **SQLite on disk** | Zero infra, embedded, survives restarts | Doesn't scale across multiple instances |
| **In-memory (`AtomicU64`)** | Simplest, no deps | Resets on restart |

**Recommendation:** Start with in-memory or SQLite. Add Supabase only if you actually need global persistence.

### Frontend Deployment
Since the frontend is vanilla HTML/CSS/JS with no build step, you can host it anywhere:
- **Vercel** / **Netlify** — free, point at the `frontend/` folder
- **GitHub Pages** — also free
- **Same Railway service** — just add the frontend files to the backend's binary or serve them as static assets

## Rust Compiler Warnings (to fix)

Run `cargo check` to see these:

1. **`entities` field never read** — `src/models/tweet.rs:33`
   - `pub entities: Option<EntityBlock>` on `SyndicationTweet` — can remove
2. **`in_reply_to_screen_name` field never read** — `src/models/tweet.rs:37`
   - `pub in_reply_to_screen_name: Option<String>` on `SyndicationTweet` — can remove
3. **`media` field never read** — `src/models/tweet.rs:66`
   - `pub media: Vec<serde_json::Value>` on `EntityBlock` — can remove
4. **`ExtendedEntities` type alias never used** — `src/models/tweet.rs:69`
   - `pub type ExtendedEntities = MediaEntities;` — can remove
5. ~~**`include_quote` and `include_reply` never read**~~ — `src/models/tweet.rs:121,123` **(FIXED)**
   - Fields on `DownloadRequest` — now implemented in all three routes (download, audio, captions)
6. **`quality` field on `DownloadRequest`** — used in download/audio routes but the captions endpoint also accepts `DownloadRequest` (which includes `quality`) without using it. Either split into separate request types or suppress.

## Known Issues & What's Not Built Yet

### ✅ Done
- Rust backend (`src/`) — all routes, services, models implemented
- Caption format conversion (VTT → SRT) — `src/services/captions.rs` with tests
- Download counter UI — frontend has `incrementStat()` and HTML counter elements
- Video preview via proxy endpoint (`GET /api/preview`) — CORS-free MP4 playback, implemented in `src/routes/preview.rs`
- Tweet card caption mode — "show captions" toggles a full X-style tweet card (avatar, name, handle, verified badge, tweet text, video, timestamp, likes) instead of plain text overlay
- Profile picture (`avatar_url`) — `SyndicationUser.profile_image_url_https` parsed and exposed on `TweetInfo`
- t.co links stripped from tweet text on the frontend in `renderResult()`
- Likes count — heart icon + formatted count in tweet card footer
- Tweet card video styling — rounded corners, border, centered in card, matches X/Twitter's inline look
- Handle font fixed to sans-serif to match X/Twitter
- **Disabled option rows** — quote/reply checkboxes greyed out (`option-row-disabled` class) when the tweet has no quoted/reply data, with `setOptionEnabled()` helper in `app.js`
- **Lazy card player loading** — `loadCardPlayer()` called only when captions mode first opens; `cardPlayerReady` flag prevents redundant loads; proper pause/resume between plain player and card player
- **Security — preview URL validation** — regex check in `preview.rs` ensures only `video.twimg.com` `.mp4` URLs are proxied
- **Security — configurable CORS** — `CORS_ORIGIN` env var restricts origins in production; falls back to `Any` in dev
- **Security — rate limiting** — `RateLimiter` struct in `main.rs` (30 req/min per key) applied to `/api/preview`; returns `429 TOO_MANY_REQUESTS`

### ✅ In Progress

### ✅ Quote/reply download merging
✅ `include_quote` / `include_reply` flags trigger fetching the quoted/reply tweet's video in all three routes
✅ Concat videos with ffmpeg via `merge_mp4s()` in `services/download.rs`
✅ Merge SRT captions via `merge_srt_captions()` in `services/download.rs`
✅ Text-only quoted tweets contribute nothing to output
✅ Smart parent promotion: if a reply tweet's parent has video, parent becomes primary content
✅ `fetch_quoted_tweet()` and `tweet_ref_from()` added to `services/twitter.rs`
✅ `id_str` on `SyndicationQuotedTweet` added for full-quote fetching
✅ `/api/info` now returns full `TweetRef` with variants for both quote and reply

### ✅ Reply + quote tweet frontend design cleanup
- Reply context moved below tweet card footer, polished inline (avatar + handle + text, no box/label)
- Quote tweet rendered in captions mode as a unified bordered box between main tweet body and footer
- "include quoted tweet" renamed to "include quote to tweet" — when ON shows the outer tweet quoting the parent tweet with both videos in one box, when OFF shows just the inner tweet

### ❌ Download counter stuck at 0
The `totalCount` stat never increments because no fetch is done server-side. The frontend runs `incrementStat("totalCount")` on download but there's no backend persistence.

### ❌ Captions-baked video download (future)
When captions mode is on and user hits download, the output MP4 should have the tweet card UI **rendered into the video frames** — not just raw video. This means using ffmpeg to composite:
- Dark background bar with tweet card layout
- Avatar image overlay
- Drawtext for handle, tweet text, timestamp, likes
- Verified badge SVG as image overlay

This is NOT what the current `merge_mp4s` in `services/download.rs` does — that just concatenates raw videos. The compositing approach will need a dedicated function in `services/download.rs` (or a new module) using ffmpeg's `drawtext`, `overlay`, and `color` filterchain.

### ❌ Overlay rendering issues (`render_card`)

1. **Performance**: downloading with captions (`render_card: true`) is slow — the ffmpeg overlay compositing with drawtext + Python Pillow asset generation takes several seconds before the download starts.

2. ~~**Vertical videos**: when the source video is portrait/tall (e.g. 1080×1920), the overlay layout calculations assume a landscape aspect ratio. The card padding and bar heights don't scale correctly, making the video too large for the frame and the caption text is pushed off-screen or clipped.~~ **(FIXED)**

3. ~~**Date/likes order**: the footer currently renders likes before the date (`"8.4K Likes · May 7, 2026"`). It should be date first, then likes (`"May 7, 2026 · 8.4K Likes"`).~~ **(FIXED)**

4. ~~**Non-Latin text rendering**: tweets containing non-English scripts (Korean, Japanese, CJK, emoji, etc.) display as placeholder/tofu characters (`□` or missing glyph boxes). The bundled Geist font doesn't cover these codepoints — need a fallback font or a broader font like Noto Sans CJK.~~ **(FIXED — Noto Sans candidates added; falls back to fontconfig when no font file found)**
