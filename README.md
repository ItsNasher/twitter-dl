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
│   ├── index.html
│   ├── css/styles.css
│   └── js/
│       ├── app.js        # UI state, event handlers, validation
│       ├── api.js        # fetch() calls to Rust backend
│       └── downloader.js # blob download helpers
├── src/
│   ├── main.rs           # Axum server entry, router
│   ├── error.rs          # AppError type
│   ├── routes/           # One file per endpoint
│   ├── services/         # Business logic (twitter API, video, captions)
│   └── models/           # Serde structs
├── Cargo.toml
├── .env                  # HOST, PORT
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
  ]
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
5. **`include_quote` and `include_reply` never read** — `src/models/tweet.rs:121,123`
   - Fields on `DownloadRequest` — the routes accept them but don't use them yet (the backend always downloads the main tweet video). Remove or implement.
6. **`quality` field on `DownloadRequest`** — used in download/audio routes but the captions endpoint also accepts `DownloadRequest` (which includes `quality`) without using it. Either split into separate request types or suppress.

## Known Issues & What's Not Built Yet

### ❌ Video preview unavailable (CORS)
The preview in `frontend/js/app.js:60` sets `<video>.src` to Twitter's CDN URL directly. Browsers block this cross-origin (no `Access-Control-Allow-Origin` header), so `onerror` fires at line 68 showing "preview unavailable."

**Fix — Option A: Proxy endpoint (full video preview)**
- Add `GET /api/preview?url=<encoded_twitter_cdn_url>` that proxies the video bytes server-side through the shared `reqwest::Client`
- In `loadVideoPreview()`, set `player.src = "http://localhost:3000/api/preview?url=" + encodeURIComponent(variant.url)`
- New file: `src/routes/preview.rs` (stream bytes, set `content-type: video/mp4` + `access-control-allow-origin: *`)
- Wire in `main.rs` router

**Fix — Option B: Thumbnail poster (static image, simpler)**
- Add `media_url_https: Option<String>` to `MediaItem` in `src/models/tweet.rs:77`
- Add `thumbnail_url: Option<String>` to `TweetInfo` in `src/models/tweet.rs:11`
- Populate in `build_tweet_info()` in `src/services/twitter.rs`
- Set `player.poster = currentTweetData.thumbnail_url` in `loadVideoPreview()`

### ❌ API contract mismatch in AGENTS.md
The `TweetInfo` response has `quoted_tweet` and `in_reply_to` (objects with `author`/`text`), NOT `is_quote`/`is_reply` booleans. The JSON example in this file is correct; the old `is_quote`/`is_reply` fields were removed from the struct.

### ❌ Download counter stuck at 0
The `totalCount` stat never increments because no fetch is done server-side. The frontend runs `incrementStat("totalCount")` on download but there's no backend persistence.

### ✅ Done
- Rust backend (`src/`) — all routes, services, models implemented
- Caption format conversion (VTT → SRT) — `src/services/captions.rs` with tests
- Download counter UI — frontend has `incrementStat()` and HTML counter elements
