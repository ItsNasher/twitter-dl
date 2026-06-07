# twitter-dl

A Twitter/X video downloader with a plain HTML/CSS/JS frontend and a Rust (Axum) backend.
Users paste a tweet URL and can download the video as MP4, captions as SRT, or audio only.
Supports tweet card overlays (captions mode) with reply and quote tweet threading.

## Stack
- **Frontend**: Vanilla HTML, CSS, JS — no framework, no bundler
- **Backend**: Rust with Axum, reqwest, tokio, serde
- **Overlay rendering**: Python + Pillow (card generation), ffmpeg (compositing)
- **No build step for frontend** — just open index.html or serve statically

## Directory Structure
```
twitter-dl/
├── fonts/
│   ├── GeistVF.ttf               # Primary UI font
│   ├── NotoSansJP-Regular.otf    # Japanese fallback
│   ├── NotoSansKR-Regular.otf    # Korean fallback
│   └── NotoColorEmoji.ttf        # Emoji fallback
├── frontend/
│   ├── css/
│   │   ├── styles.css            # Design tokens, layout, dark theme
│   │   └── tweet.css             # Tweet card, captions mode styles
│   ├── js/
│   │   ├── api.js                # fetch() calls to Rust backend
│   │   ├── app.js                # UI state, event handlers, validation
│   │   └── downloader.js         # blob download helpers
│   └── index.html
├── src/
│   ├── main.rs                   # Axum server entry, router, rate limiter
│   ├── error.rs                  # AppError type
│   ├── models/
│   │   ├── mod.rs
│   │   ├── tweet.rs              # Syndication API response structs
│   │   └── video_variant.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── audio.rs              # POST /api/audio
│   │   ├── captions.rs           # POST /api/captions
│   │   ├── download.rs           # POST /api/download
│   │   ├── info.rs               # POST /api/info
│   │   └── preview.rs            # GET /api/preview (video proxy)
│   └── services/
│       ├── mod.rs
│       ├── captions.rs           # VTT→SRT conversion
│       ├── download.rs           # MP4/SRT merging helpers
│       ├── overlay.rs            # Pillow card rendering, ffmpeg compositing
│       ├── twitter.rs            # Syndication API calls, tweet parsing
│       └── video.rs              # HLS segment merging, ffmpeg remux
├── .env                          # HOST, PORT, CORS_ORIGIN
├── .env.example
├── .gitignore
├── apt.txt                       # Railway system deps (ffmpeg, python3, etc.)
├── Cargo.lock
├── Cargo.toml
├── railway.toml                  # Railway deploy config
├── AGENTS.md
└── README.md
```

## API Contract

| Method | Endpoint        | Body                                                              | Returns         |
|--------|-----------------|-------------------------------------------------------------------|-----------------|
| POST   | /api/info       | `{ url }`                                                         | TweetInfo JSON  |
| POST   | /api/download   | `{ url, quality, include_quote, include_reply, render_card }`    | MP4 blob        |
| POST   | /api/captions   | `{ url, include_quote, include_reply }`                           | SRT blob        |
| POST   | /api/audio      | `{ url, quality, include_quote, include_reply }`                  | M4A blob        |
| GET    | /api/preview    | `?url=<video_url>`                                                | MP4 proxy stream|

### TweetInfo response shape
```json
{
  "author": "username",
  "display_name": "Display Name",
  "created_at": "May 7, 2025",
  "text": "tweet content...",
  "quoted_tweet": null | { "author": "...", "display_name": "...", "text": "...", "variants": [...] },
  "in_reply_to": null | { "author": "...", "display_name": "...", "text": "...", "variants": [...] },
  "variants": [
    { "label": "1080p", "url": "...", "bitrate": 2176000 },
    { "label": "720p",  "url": "...", "bitrate": 832000  }
  ],
  "avatar_url": null | "https://pbs.twimg.com/profile_images/..._normal.jpg",
  "likes": null | 1234
}
```

## Running the Project

### Prerequisites
- Rust (stable)
- Python 3 + Pillow (`pip install Pillow`)
- ffmpeg (in PATH)

### Backend
```bash
cp .env.example .env       # optional — defaults to 127.0.0.1:3000
cargo run                  # starts on localhost:3000
```

### Frontend
```bash
# No build step needed
npx serve frontend         # or open frontend/index.html directly in browser
```

## Features

### Download modes
- **Raw MP4** — streams directly from Twitter CDN, no processing
- **Audio only** — extracts M4A audio track via ffmpeg
- **Captions (SRT)** — converts VTT subtitle track to SRT format

### Captions mode (tweet card overlay)
When "show captions" is enabled, the downloaded MP4 has the tweet card rendered directly into the video frames using Python Pillow + ffmpeg compositing:
- Avatar, display name, handle, verified badge, X logo
- Tweet body text with per-character CJK/emoji font fallback
- Rounded video with border
- Footer: date · ♡ likes
- 2× supersampling (SSAA) for crisp text at all resolutions
- Hardware encoder detection (NVENC, AMF, QSV, libx264 fallback)

### Reply thread mode (captions on)
When a reply tweet is fetched and both captions + reply are enabled:
- Thread line connects main tweet avatar through video to reply avatar
- Main tweet card on top, reply card below
- Parent tweet footer shown above reply section

### Quote tweet mode (captions on)
When a quote tweet is fetched and both captions + quote are enabled:
- Outer tweet card on top with body text
- Quoted tweet's video plays in the middle
- Quote box below showing quoted tweet author, handle, date, text
- Outer tweet footer: date · ♡ likes

## Key Conventions

### Rust
- Use `AppError` (defined in `error.rs`) for all route error returns
- Services return `Result<T, AppError>`, routes just `?`-propagate
- All external HTTP calls go through a shared `reqwest::Client` via Axum state
- HLS segment merging in `services/video.rs` — shells out to ffmpeg for remux
- Overlay pipeline in `services/overlay.rs` — generates Pillow PNGs, composites with ffmpeg

### Frontend (JS)
- `app.js` owns all UI state — do not manipulate DOM from `api.js` or `downloader.js`
- `api.js` is pure fetch functions only — no DOM, no state
- `downloader.js` is pure blob/download helpers — no DOM, no state
- Reply/quote options only become selectable after captions is enabled
- URL validation regex: `/^https?:\/\/(twitter\.com|x\.com)\/.+\/status\/\d+/`

### CSS
- All design tokens are CSS variables at `:root` in `styles.css` — never hardcode colors
- Font: JetBrains Mono for monospace elements, Geist for body text
- Color palette is Zed-inspired dark theme

### Fonts
Bundled in `fonts/` and loaded by the overlay Python scripts at render time:
- **GeistVF.ttf** — primary UI font (Latin)
- **NotoSansJP / NotoSansKR** — CJK fallback per character
- **NotoColorEmoji** — emoji fallback per character

The overlay uses per-character script detection (`char_script()`) to pick the right font for each glyph, so Japanese, Korean, Chinese, and emoji all render correctly.

## Environment Variables
```
HOST=127.0.0.1        # default 127.0.0.1 — use 0.0.0.0 for Railway
PORT=3000             # default 3000
CORS_ORIGIN=          # restrict CORS in production (omit for dev = allow all)
```

## Deployment

### Backend — Railway
The Rust backend runs as a single binary. Railway detects Rust automatically.

1. Push repo to GitHub
2. Connect repo to [Railway](https://railway.app)
3. Set environment variables:
   ```
   HOST=0.0.0.0
   PORT=3000
   CORS_ORIGIN=https://your-frontend.vercel.app
   ```
4. Railway builds with `cargo build --release` and starts the server
5. System deps (ffmpeg, python3, fonts) are installed from `apt.txt` at build time

`apt.txt`:
```
ffmpeg
python3
python3-pip
fonts-noto-color-emoji
```

`railway.toml`:
```toml
[build]
builder = "NIXPACKS"

[deploy]
startCommand = "pip install Pillow --break-system-packages && cargo run --release"
```

### Frontend — Vercel
1. Go to [vercel.com](https://vercel.com), sign in with GitHub
2. Import the `twitter-dl` repo
3. Set **Root Directory** to `frontend`
4. Leave build command and output directory empty
5. Deploy — get `twitter-dl.vercel.app`
6. Add a custom domain in Vercel project settings if desired

Before deploying, update `api.js`:
```javascript
const API_BASE = 'https://your-backend.up.railway.app/api';
```

### Custom Domain
Point your domain's DNS at Vercel (frontend) or Railway (if serving everything from one service). Both handle SSL automatically.

## Known Issues & Roadmap

### ✅ Done
- All download routes (MP4, SRT, M4A)
- VTT → SRT caption conversion with tests
- Video preview proxy (`/api/preview`) with rate limiting and URL validation
- Tweet card overlay (captions mode) — avatar, name, verified, body, footer
- Reply thread overlay — thread line, parent/reply cards, footer
- Quote tweet overlay — outer card, inner video, quote box, footer
- Per-character CJK and emoji font fallback in overlay rendering
- 2× SSAA supersampling for overlay text quality
- Hardware encoder detection (NVENC → AMF → QSV → libx264)
- Smart parent promotion for reply tweets (parent video shown as main)
- Reply/quote options locked behind captions toggle
- Download counters (frontend only, in-memory)
- CORS restriction via `CORS_ORIGIN` env var
- Rate limiting on preview endpoint

### ❌ Download counter persistence
Counters reset on server restart. Needs a database (Supabase/SQLite) with:
- `GET /api/stats` — return current counts
- `POST /api/stats/increment` — increment a counter
- Frontend updated to call these instead of just updating DOM