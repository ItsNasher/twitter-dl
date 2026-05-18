use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::process::Command;

use bytes::Bytes;
use reqwest::Client;

use crate::error::AppError;
use crate::models::TweetRef;

const BASE_WIDTH: f64 = 600.0;
const MAX_RENDER_W: i32 = 1280;

// video encoder detection

enum VideoEncoder {
    Nvenc,
    Amf,
    Qsv,
    Libx264,
}

static BEST_ENCODER: OnceLock<VideoEncoder> = OnceLock::new();

pub fn init_encoder() {
    let test_encoder = |enc: &str| -> bool {
        std::process::Command::new("ffmpeg")
            .args([
                "-v", "quiet",
                "-f", "lavfi",
                "-i", "color=black:s=64x64:d=0.1",
                "-c:v", enc,
                "-f", "null",
                "-",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };

    BEST_ENCODER.get_or_init(|| {
        if test_encoder("h264_nvenc") { return VideoEncoder::Nvenc; }
        if test_encoder("h264_amf")   { return VideoEncoder::Amf; }
        if test_encoder("h264_qsv")   { return VideoEncoder::Qsv; }

        VideoEncoder::Libx264
    });
}

fn best_encoder() -> &'static VideoEncoder {
    BEST_ENCODER.get().expect("init_encoder must be called before best_encoder")
}

fn encoder_args() -> Vec<String> {
    match best_encoder() {
        VideoEncoder::Nvenc => vec![
            "-c:v".into(), "h264_nvenc".into(),
            "-preset".into(), "p4".into(),
            "-cq".into(), "18".into(),
            "-rc".into(), "vbr".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
        VideoEncoder::Amf => vec![
            "-c:v".into(), "h264_amf".into(),
            "-quality".into(), "quality".into(),
            "-qp_i".into(), "18".into(),
            "-qp_p".into(), "18".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
        VideoEncoder::Qsv => vec![
            "-c:v".into(), "h264_qsv".into(),
            "-preset".into(), "fast".into(),
            "-global_quality".into(), "18".into(),
            "-pix_fmt".into(), "yuv420p".into(),
        ],
        VideoEncoder::Libx264 => vec![
            "-c:v".into(), "libx264".into(),
            "-preset".into(), "fast".into(),
            "-crf".into(), "20".into(),
            "-threads".into(), "0".into(),
        ],
    }
}

// public entry point

pub async fn apply_tweet_overlay(
    client: &Client,
    video_bytes: Bytes,
    tweet: &TweetRef,
    tweet_id: &str,
) -> Result<Bytes, AppError> {
    let dir = std::env::temp_dir().join(format!("twdl_overlay_{}", tweet_id));
    let _ = std::fs::create_dir_all(&dir);

    let video_path = dir.join("video.mp4");
    std::fs::write(&video_path, &video_bytes)
        .map_err(|e| AppError::Internal(e.into()))?;

    // raw dimensions
    let (vid_w_raw, vid_h_raw) = probe_video_dims(&video_path).await?;

    let (vid_w, vid_h) = if vid_w_raw > MAX_RENDER_W {
        let h = ((vid_h_raw as f64 * MAX_RENDER_W as f64 / vid_w_raw as f64) / 2.0).round() as i32 * 2;
        (MAX_RENDER_W, h)
    } else {
        let h = if vid_h_raw % 2 == 0 { vid_h_raw } else { vid_h_raw + 1 };
        (vid_w_raw, h)
    };
    let needs_scale = vid_w != vid_w_raw || vid_h != vid_h_raw;

    let sf = (vid_w as f64 / BASE_WIDTH).max(0.5);

    // avatar
    let avatar_src_path = dir.join("avatar_src.jpg");
    let has_avatar = if let Some(ref url) = tweet.avatar_url {
        let hd_url = url.replace("_normal.", "_200x200.");
        let ok = download_file(client, &hd_url, &avatar_src_path).await.is_ok();
        if !ok { download_file(client, url, &avatar_src_path).await.is_ok() } else { true }
    } else {
        false
    };

    // layout
    let pad_h      = sc(16.0, sf);
    let pad_top    = sc(18.0, sf);
    let ava_size   = sc(42.0, sf);
    let ava_gap    = sc(10.0, sf);
    let name_fs    = sc(15.0, sf);
    let handle_fs  = sc(13.0, sf);
    let check_sz   = sc(16.0, sf);
    let xlogo_sz   = sc(20.0, sf);
    let body_fs    = sc(17.0, sf);
    let body_lh    = sc(6.0,  sf);
    let footer_fs  = sc(13.0, sf);
    let heart_sz   = sc(14.0, sf);
    let vid_pad_h  = sc(24.0, sf);
    let vid_corner = sc(20.0, sf);
    let sec_gap    = sc(14.0, sf);
    let footer_pad = sc(14.0, sf);

    // body text
    let body_clean   = strip_tco(&tweet.text);
    let max_chars    = ((vid_w as f64 - pad_h as f64 * 2.0) / (body_fs as f64 * 0.55)).max(10.0) as usize;
    let wrapped      = word_wrap(&body_clean, max_chars, 5);
    let body_lines   = wrapped.lines().count().max(1) as i32;
    let body_block_h = body_lines * (body_fs + body_lh) - body_lh;

    // section heights
    let header_h = pad_top + ava_size + sec_gap;
    let body_h   = body_block_h + sec_gap;
    let top_bar  = header_h + body_h;
    let bot_bar  = sec_gap + footer_fs + footer_pad;

    // video display area
    let vid_dw    = vid_w - vid_pad_h * 2;
    let display_h = {
        let raw = vid_h as f64 * vid_dw as f64 / vid_w as f64;
        let r   = raw.round() as i32;
        if r % 2 == 0 { r } else { r + 1 }
    };
    let total_h = top_bar + display_h + bot_bar;

    // footer text
    let likes_str = tweet.likes.map(|n| format_count(n)).unwrap_or_default();
    let has_likes = tweet.likes.is_some() && !likes_str.is_empty();
    let footer_date  = tweet.created_at.trim().to_string();
    let footer_likes = if has_likes { format!("{} Likes", likes_str) } else { String::new() };

    // font path
    let font_path = find_font()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .unwrap_or_default();

    // generate all assets
    let card_top_path  = dir.join("card_top.png");
    let card_bot_path  = dir.join("card_bot.png");
    let mask_path      = dir.join("video_mask.png");
    let avatar_path    = dir.join("avatar_circle.png");

    generate_cards(
        &dir,
        &card_top_path,
        &card_bot_path,
        &mask_path,
        &avatar_path,
        if has_avatar { Some(&avatar_src_path) } else { None },
        // dimensions
        vid_w, top_bar, bot_bar,
        vid_dw, display_h, vid_corner,
        // layout
        pad_h, pad_top, ava_size, ava_gap,
        name_fs, handle_fs, check_sz, xlogo_sz,
        body_fs, body_lh, footer_fs, heart_sz,
        sec_gap, header_h,
        // text content
        &tweet.display_name,
        &tweet.author,
        &wrapped,
        &footer_date,
        &footer_likes,
        // font
        &font_path,
    ).await?;

    // ── Build filter_complex ───────────────────────────────────────────────
    //
    // Inputs:
    //   [0:v]  source video
    //   [1:v]  card_top.png   (vid_w × top_bar, RGBA)
    //   [2:v]  card_bot.png   (vid_w × bot_bar, RGBA)
    //   [3:v]  video_mask.png (vid_dw × display_h, grayscale L)
    //
    // No drawtext at all — Pillow drew everything already.

    let mut f = String::new();

    // 1. Optional source downscale
    if needs_scale {
        f.push_str(&format!(
            "[0:v]scale=w={vw}:h={vh}:flags=lanczos[src];",
            vw=vid_w, vh=vid_h
        ));
    } else {
        f.push_str("[0:v]copy[src];");
    }

    // 2. Scale video to padded display area (exact dims to match mask)
    f.push_str(&format!(
        "[src]scale=w={dw}:h={dh}:flags=lanczos[scaled];",
        dw=vid_dw, dh=display_h
    ));

    // 3. Rounded corners via alphamerge
    f.push_str("[scaled][3:v]alphamerge[rounded];");

    // 4. Black canvas
    f.push_str(&format!(
        "color=c=black:s={vw}x{th}[bg];",
        vw=vid_w, th=total_h
    ));

    // 5. Place rounded video onto canvas at (vid_pad_h, top_bar)
    f.push_str(&format!(
        "[bg][rounded]overlay=x={vx}:y={tb}[with_video];",
        vx=vid_pad_h, tb=top_bar
    ));

    // Scale 2× card images back to actual size
    f.push_str(&format!(
        "[1:v]scale=w={vw}:h={tb}:flags=lanczos[top_ds];",
        vw = vid_w, tb = top_bar
    ));
    f.push_str(&format!(
        "[2:v]scale=w={vw}:h={bb}:flags=lanczos[bot_ds];",
        vw = vid_w, bb = bot_bar
    ));

    // 6. Overlay top card
    f.push_str("[with_video][top_ds]overlay=x=0:y=0[with_top];");

    // 7. Overlay bottom card
    f.push_str(&format!(
        "[with_top][bot_ds]overlay=x=0:y={bot_y}[final];",
        bot_y = top_bar + display_h
    ));

    let out_path = dir.join("output.mp4");

    let mut args: Vec<String> = vec![
        "-y".to_string(),
        "-i".to_string(), video_path.to_str().unwrap().to_string(),
        "-i".to_string(), card_top_path.to_str().unwrap().to_string(),
        "-i".to_string(), card_bot_path.to_str().unwrap().to_string(),
        "-i".to_string(), mask_path.to_str().unwrap().to_string(),
        "-filter_complex".to_string(), f,
        "-map".to_string(), "[final]".to_string(),
        "-map".to_string(), "0:a?".to_string(),
    ];

    args.extend(encoder_args());

    args.extend([
        "-c:a".to_string(), "copy".to_string(),
        "-movflags".to_string(), "+faststart".to_string(),
        "-shortest".to_string(),
        out_path.to_str().unwrap().to_string(),
    ]);

    let child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Ffmpeg(format!("failed to spawn ffmpeg: {}", e)))?;

    let output = child.wait_with_output().await
        .map_err(|e| AppError::Ffmpeg(format!("ffmpeg wait failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!("ffmpeg overlay failed:\n{}", stderr);
        return Err(AppError::Ffmpeg(format!(
            "ffmpeg overlay exited with code {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    let result = std::fs::read(&out_path)
        .map_err(|e| AppError::Internal(e.into()))?;

    if result.is_empty() {
        return Err(AppError::Ffmpeg("ffmpeg overlay produced no output".into()));
    }

    let _ = std::fs::remove_dir_all(&dir);
    Ok(Bytes::from(result))
}

// python/pillow

#[allow(clippy::too_many_arguments)]
async fn generate_cards(
    dir: &Path,
    card_top_path: &Path,
    card_bot_path: &Path,
    mask_path: &Path,
    avatar_out_path: &Path,
    avatar_src: Option<&Path>,
    // dims
    vid_w: i32, top_bar: i32, bot_bar: i32,
    vid_dw: i32, display_h: i32, vid_corner: i32,
    // layout
    pad_h: i32, pad_top: i32, ava_size: i32, ava_gap: i32,
    name_fs: i32, handle_fs: i32, check_sz: i32, xlogo_sz: i32,
    body_fs: i32, body_lh: i32, footer_fs: i32, heart_sz: i32,
    sec_gap: i32, header_h: i32,
    // text
    display_name: &str,
    author: &str,
    body: &str,
    footer_date: &str,
    footer_likes: &str,
    font_path: &str,
) -> Result<(), AppError> {
    let av_src_str = avatar_src
        .map(|p| format!("r'{}'", p.display().to_string().replace('\\', "/")))
        .unwrap_or_else(|| "None".to_string());

    let py_str = |s: &str| -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n")
    };

    let script = format!(
        r#"
import os, math
from PIL import Image, ImageDraw, ImageFont

SSAA = 2  # supersample factor — render cards at 2× then downscale in ffmpeg

# font
FONT_PATH = r'{font_path}'

def load_font(size):
    if FONT_PATH and os.path.exists(FONT_PATH):
        try: return ImageFont.truetype(FONT_PATH, size)
        except Exception: pass
    candidates = [
        '/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc',
        '/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc',
        '/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc',
        '/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf',
        '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
        '/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf',
        'C:/Windows/Fonts/segoeui.ttf',
        'C:/Windows/Fonts/arial.ttf',
    ]
    for p in candidates:
        if os.path.exists(p):
            try: return ImageFont.truetype(p, size)
            except Exception: pass
    # fontconfig last resort — finds whatever the system has for CJK
    try:
        import subprocess
        for lang in ['ja', 'ko', 'zh', '']:
            query = (':lang=' + lang) if lang else 'NotoSans'
            r = subprocess.run(
                ['fc-match', query, '--format=%{{file}}'],
                capture_output=True, text=True, timeout=3
            )
            if r.returncode == 0:
                p = r.stdout.strip()
                if p and os.path.exists(p):
                    try: return ImageFont.truetype(p, size)
                    except Exception: pass
    except Exception:
        pass
    return ImageFont.load_default()

# avatar
def make_avatar(size, src_path, out_path):
    if src_path is not None and os.path.exists(src_path):
        try:
            base = Image.open(src_path).convert('RGBA').resize((size, size), Image.LANCZOS)
        except Exception:
            base = Image.new('RGBA', (size, size), (29, 34, 48, 255))
    else:
        base = Image.new('RGBA', (size, size), (29, 34, 48, 255))
    mask = Image.new('L', (size, size), 0)
    ImageDraw.Draw(mask).ellipse([0, 0, size-1, size-1], fill=255)
    out = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    out.paste(base, mask=mask)
    out.save(out_path, 'PNG')

# checkmark badge
def draw_checkmark(canvas, x, y, size):
    d = ImageDraw.Draw(canvas)
    d.ellipse([x, y, x+size-1, y+size-1], fill=(29, 155, 240, 255))
    p = size / 16.0
    lw = max(1, round(1.9 * p))
    pts = [
        (x + round(3.5*p), y + round(8.5*p)),
        (x + round(6.5*p), y + round(11.5*p)),
        (x + round(12.5*p), y + round(4.5*p)),
    ]
    d.line([pts[0], pts[1]], fill='white', width=lw)
    d.line([pts[1], pts[2]], fill='white', width=lw)

# x logo
def draw_xlogo(canvas, ox, oy, size):
    s = size / 24.0
    def pt(px, py):
        return (ox + px * s, oy + py * s)

    # Outer silhouette — the full X shape (from the official SVG path on a 24×24 viewbox)
    outer = [
        pt(18.244, 2.25),  pt(21.552, 2.25),  pt(14.325, 10.51),
        pt(22.827, 22.5),  pt(16.17,  22.5),  pt(11.455, 16.307),
        pt(6.061,  22.5),  pt(2.752,  22.5),  pt(10.032, 14.175),
        pt(2.752,  3.248), pt(8.844,  3.248),  pt(13.107,  8.886),
        pt(18.244, 3.248),
    ]
    # Inner diagonal quad — the cutout between the two bars
    inner = [
        pt(17.083, 20.61), pt(18.916, 20.61),
        pt(7.084,   4.126), pt(5.117,   4.126),
    ]

    d = ImageDraw.Draw(canvas)
    d.polygon(outer, fill=(255, 255, 255, 220))
    d.polygon(inner, fill=(0, 0, 0, 255))  # black punches out the cutout against the card bg

# heart icon
def draw_heart(canvas, x, y, size):
    d = ImageDraw.Draw(canvas)
    color = (113, 118, 123, 255)
    xs, ys = [], []
    for i in range(360):
        t = math.radians(i)
        xs.append(16 * math.sin(t)**3)
        ys.append(-(13*math.cos(t) - 5*math.cos(2*t) - 2*math.cos(3*t) - math.cos(4*t)))
    minx, maxx = min(xs), max(xs)
    miny, maxy = min(ys), max(ys)
    sc = (size * 0.82) / (maxx - minx)
    cx = x + size/2
    cy = y + size/2 + size*0.04
    poly = [(cx+(px-(minx+maxx)/2)*sc, cy+(py-(miny+maxy)/2)*sc) for px,py in zip(xs,ys)]
    lw = max(1, round(size / 11))
    d.line(poly + [poly[0]], fill=color, width=lw)

# video mask
def make_video_mask(w, h, r, path):
    mask = Image.new('L', (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, w-1, h-1], radius=r, fill=255)
    mask.save(path, 'PNG')

# text helpers
def text_width(font, text):
    try:
        bb = font.getbbox(text)
        return bb[2] - bb[0]
    except Exception:
        return len(text) * font.size

def draw_multiline(d, text, font, x, y, color, line_height):
    for line in text.split('\n'):
        d.text((x, y), line, font=font, fill=color)
        y += line_height

# card top
def make_card_top(path):
    W, H = {vid_w} * SSAA, {top_bar} * SSAA
    img = Image.new('RGBA', (W, H), (0, 0, 0, 255))
    d   = ImageDraw.Draw(img)

    font_name   = load_font({name_fs}   * SSAA)
    font_handle = load_font({handle_fs} * SSAA)
    font_body   = load_font({body_fs}   * SSAA)

    ava_img = Image.open(r'{av_out}').convert('RGBA')
    img.paste(ava_img, ({pad_h} * SSAA, {pad_top} * SSAA), ava_img)

    name_x = ({pad_h} + {ava_size} + {ava_gap}) * SSAA
    name_y = ({pad_top} + ({ava_size} - {name_fs} - {handle_fs} - 2) // 2) * SSAA
    d.text((name_x, name_y), '{display_name}', font=font_name, fill=(255,255,255,255))

    name_w  = text_width(font_name, '{display_name}')
    check_x = name_x + name_w + 3 * SSAA
    check_y = name_y + ({name_fs} * SSAA - {check_sz} * SSAA) // 2 + round({name_fs} * SSAA * 0.12)
    draw_checkmark(img, check_x, check_y, {check_sz} * SSAA)

    handle_y = name_y + {name_fs} * SSAA + 3 * SSAA
    d.text((name_x, handle_y), '@{author}', font=font_handle, fill=(113,118,123,255))

    xlogo_x = W - ({pad_h} + {xlogo_sz}) * SSAA
    xlogo_y = ({pad_top} + ({ava_size} - {xlogo_sz}) // 2) * SSAA
    draw_xlogo(img, xlogo_x, xlogo_y, {xlogo_sz} * SSAA)

    body_y = {header_h} * SSAA
    lh     = ({body_fs} + {body_lh}) * SSAA
    draw_multiline(d, '{body}', font_body, {pad_h} * SSAA, body_y, (255,255,255,255), lh)

    img.save(path, 'PNG')

# card bottom
def make_card_bot(path):
    W, H = {vid_w} * SSAA, {bot_bar} * SSAA
    img  = Image.new('RGBA', (W, H), (0, 0, 0, 255))
    font_footer = load_font({footer_fs} * SSAA)
    d    = ImageDraw.Draw(img)
    text_y = {sec_gap} * SSAA
    cur_x  = {pad_h}   * SSAA

    date_str  = '{footer_date}'
    likes_str = '{footer_likes}'

    d.text((cur_x, text_y), date_str, font=font_footer, fill=(113,118,123,255))
    cur_x += text_width(font_footer, date_str)

    if likes_str:
        sep = '  \u00b7  '
        d.text((cur_x, text_y), sep, font=font_footer, fill=(113,118,123,255))
        cur_x += text_width(font_footer, sep)
        heart_y_pos = {sec_gap} * SSAA + ({footer_fs} * SSAA - {heart_sz} * SSAA) // 2
        draw_heart(img, cur_x, heart_y_pos, {heart_sz} * SSAA)
        cur_x += {heart_sz} * SSAA + 4 * SSAA
        d.text((cur_x, text_y), likes_str, font=font_footer, fill=(113,118,123,255))

    img.save(path, 'PNG')

# run everything
make_avatar({ava_size} * SSAA, {av_src}, r'{av_out}')
make_card_top(r'{card_top}')
make_card_bot(r'{card_bot}')
make_video_mask({vid_dw}, {display_h}, {vid_corner}, r'{mask}')
print('ok')
"#,
        font_path    = font_path,
        vid_w        = vid_w,
        top_bar      = top_bar,
        bot_bar      = bot_bar,
        pad_h        = pad_h,
        pad_top      = pad_top,
        ava_size     = ava_size,
        ava_gap      = ava_gap,
        name_fs      = name_fs,
        handle_fs    = handle_fs,
        check_sz     = check_sz,
        xlogo_sz     = xlogo_sz,
        body_fs      = body_fs,
        body_lh      = body_lh,
        footer_fs    = footer_fs,
        heart_sz     = heart_sz,
        sec_gap      = sec_gap,
        header_h     = header_h,
        vid_dw       = vid_dw,
        display_h    = display_h,
        vid_corner   = vid_corner,
        display_name = py_str(display_name),
        author       = py_str(author),
        body         = py_str(body),
        footer_date  = py_str(&footer_date),
        footer_likes = py_str(&footer_likes),
        av_src       = av_src_str,
        av_out       = avatar_out_path.display().to_string().replace('\\', "/"),
        card_top     = card_top_path.display().to_string().replace('\\', "/"),
        card_bot     = card_bot_path.display().to_string().replace('\\', "/"),
        mask         = mask_path.display().to_string().replace('\\', "/"),
    );

    let script_path = dir.join("gen_cards.py");
    std::fs::write(&script_path, script.as_bytes())
        .map_err(|e| AppError::Internal(e.into()))?;

    let run = |cmd: &str| {
        let s = script_path.to_str().unwrap().to_string();
        let c = cmd.to_string();
        async move {
            Command::new(&c)
                .arg(&s)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        }
    };

    let out = match run("python").await {
        Ok(o) if o.status.success() => o,
        _ => run("python3").await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("python/python3 not found: {}", e)))?,
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::Internal(anyhow::anyhow!("card generation failed:\n{}", stderr)));
    }

    Ok(())
}

// helpers

async fn probe_video_dims(path: &Path) -> Result<(i32, i32), AppError> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
            path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("ffprobe failed: {}", e)))?;

    let out = String::from_utf8_lossy(&output.stdout);
    let mut parts = out.trim().split(',');
    let w = parts.next().and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| AppError::Ffmpeg("failed to parse video width".into()))?;
    let h = parts.next().and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| AppError::Ffmpeg("failed to parse video height".into()))?;
    Ok((w, h))
}

async fn download_file(client: &Client, url: &str, path: &Path) -> Result<(), AppError> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::TwitterApi(format!("download returned {}", resp.status())));
    }
    let bytes = resp.bytes().await?;
    std::fs::write(path, &bytes).map_err(|e| AppError::Internal(e.into()))
}

fn sc(px: f64, sf: f64) -> i32 { (px * sf).round() as i32 }

fn strip_tco(text: &str) -> String {
    text.split_whitespace()
        .filter(|w| !w.starts_with("https://t.co/") && !w.starts_with("http://t.co/"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_wrap(text: &str, max_chars: usize, max_lines: usize) -> String {
    let cleaned: String = text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    let cleaned = cleaned.trim().to_string();
    let mut result = String::new();
    let mut lines = 0;

    for paragraph in cleaned.split('\n') {
        if lines >= max_lines { break; }
        let paragraph = paragraph.trim();
        if paragraph.is_empty() { continue; }

        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.len() + word.len() + 1 > max_chars && !line.is_empty() {
                if lines >= max_lines - 1 {
                    let idx = line.len().min(max_chars.saturating_sub(3));
                    result.push_str(&line[..idx]);
                    result.push_str("...");
                    return result;
                }
                result.push_str(&line);
                result.push('\n');
                lines += 1;
                line = word.to_string();
            } else {
                if !line.is_empty() { line.push(' '); }
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            if lines >= max_lines { break; }
            result.push_str(&line);
            result.push('\n');
            lines += 1;
        }
    }
    result.trim_end().to_string()
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { n.to_string() }
}

fn find_font() -> Option<PathBuf> {
    let candidates = [
        "fonts/Geist-Regular.ttf",
        "fonts/Geist-Regular.otf",
        "fonts/Geist.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() { return Some(p); }
    }
    None
}
