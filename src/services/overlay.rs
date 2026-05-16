use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use bytes::Bytes;
use reqwest::Client;

use crate::error::AppError;
use crate::models::TweetRef;

const BASE_WIDTH: f64 = 600.0;

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

    let (width, height) = probe_video_dims(&video_path).await?;
    let sf = (width as f64 / BASE_WIDTH).max(0.5);
    let sf32 = sf as f32;

    // avatar
    let avatar_path = dir.join("avatar.jpg");
    let has_avatar = if let Some(ref url) = tweet.avatar_url {
        download_avatar(client, url, &avatar_path).await.is_ok()
    } else {
        false
    };

    if !has_avatar {
        let size = (48.0 * sf) as i32;
        let _ = make_placeholder_avatar(&avatar_path, size).await;
    }

    // word-wrap body text
    let max_chars = ((width as f64 - 32.0 * sf) / (15.0 * sf * 0.55)).max(10.0) as usize;
    let wrapped = word_wrap(&tweet.text, max_chars, 5);

    // layout
    let avatar_size = (48.0 * sf32) as i32;
    let avatar_x = (16.0 * sf32) as i32;
    let avatar_y = (14.0 * sf32) as i32;
    let name_x = (avatar_x as f64 + avatar_size as f64 + 10.0 * sf) as i32;
    let name_fs = (15.0 * sf32) as i32;
    let name_y = (14.0 * sf32) as i32;
    let handle_fs = (13.0 * sf32) as i32;
    let handle_y = (name_y as f64 + 18.0 * sf) as i32;
    let body_fs = (15.0 * sf32) as i32;
    let body_x = (16.0 * sf32) as i32;
    let body_y = (avatar_y as f64 + avatar_size as f64 + 8.0 * sf) as i32;
    let body_lh = (4.0 * sf32) as i32;
    let footer_fs = (13.0 * sf32) as i32;
    let footer_x = (16.0 * sf32) as i32;
    let num_body_lines = wrapped.lines().count().max(1) as f64;
    let body_block_h = num_body_lines * (body_fs as f64 + body_lh as f64) - body_lh as f64;
    let footer_y = (body_y as f64 + body_block_h + 10.0 * sf) as i32;
    let bottom_pad = (12.0 * sf32) as i32;
    let footer_h = footer_fs + bottom_pad;
    let bar_height = footer_y + footer_h;
    let footer_text = format_footer(tweet);

    // font
    let font = find_font();
    let font_arg = if let Some(ref fp) = font {
        format!("fontfile={}:", fp.display())
    } else {
        "font='sans-serif':".to_string()
    };

    let name_text = escape_text_value(&tweet.display_name);
    let handle_text = escape_text_value(&format!("@{}", tweet.author));
    let body_text = escape_text_value(&wrapped);
    let footer_val = escape_text_value(&footer_text);

    let filter = format!(
        "[0:v]pad=w=iw:h=ih+{bar}:y={bar}:color=#14161A[padded];\
         [1:v]scale={asiz}:{asiz}:flags=lanczos,format=rgba,\
         geq=lum='p(X,Y)':a='if(lte(sqrt((X-W/2)^2+(Y-H/2)^2),W/2),255,0)'[ava];\
         [padded][ava]overlay=x={ax}:y={ay}[a0];\
         [a0]drawtext={fa}text='{nt}':fontcolor=#CDD6F4:fontsize={nfs}:x={nx}:y={ny}[a1];\
         [a1]drawtext={fa}text='{ht}':fontcolor=#8B92A8:fontsize={hfs}:x={nx}:y={hy}[a2];\
         [a2]drawtext={fa}text='{bt}':fontcolor=#CDD6F4:fontsize={bfs}:x={bx}:y={by}:\
         line_spacing={blh},\
         drawtext={fa}text='{fv}':fontcolor=#8B92A8:fontsize={ffs}:x={fx}:y={fy}[a3]",
        bar  = bar_height,
        asiz = avatar_size,
        ax   = avatar_x,
        ay   = avatar_y,
        fa   = font_arg,
        nt   = name_text,   nfs = name_fs,   nx = name_x,   ny = name_y,
        ht   = handle_text, hfs = handle_fs,                hy = handle_y,
        bt   = body_text,   bfs = body_fs,   bx = body_x,   by = body_y,
        blh  = body_lh,
        fv   = footer_val,  ffs = footer_fs, fx = footer_x, fy = footer_y,
    );

    let mut args = Vec::<String>::new();
    args.push("-y".to_string());
    args.push("-i".to_string());
    args.push(video_path.to_str().unwrap().to_string());
    args.push("-i".to_string());
    args.push(avatar_path.to_str().unwrap().to_string());
    args.push("-filter_complex".to_string());
    args.push(filter);
    args.push("-map".to_string());
    args.push("[a3]".to_string());
    args.push("-map".to_string());
    args.push("0:a?".to_string());
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-preset".to_string());
    args.push("fast".to_string());
    args.push("-c:a".to_string());
    args.push("copy".to_string());
    args.push("-movflags".to_string());
    args.push("+faststart".to_string());
    let out_path = dir.join("output.mp4");
    args.push(out_path.to_str().unwrap().to_string());

    let child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Ffmpeg(format!("failed to spawn ffmpeg for overlay: {}", e)))?;

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("ffmpeg overlay wait failed: {}", e)))?;

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

async fn probe_video_dims(path: &Path) -> Result<(i32, i32), AppError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("ffprobe failed: {}", e)))?;

    let out = String::from_utf8_lossy(&output.stdout);
    let mut dims = out.trim().split(',');
    let w = dims
        .next()
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| AppError::Ffmpeg("failed to parse video width".into()))?;
    let h = dims
        .next()
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| AppError::Ffmpeg("failed to parse video height".into()))?;
    Ok((w, h))
}

async fn download_avatar(client: &Client, url: &str, path: &Path) -> Result<(), AppError> {
    let resp = client.get(url).send().await?;
    let bytes = resp.bytes().await?;
    std::fs::write(path, &bytes).map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}

async fn make_placeholder_avatar(path: &Path, size: i32) -> Result<(), AppError> {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("color=c=#1D2230:s={}x{}:d=0.1", size, size),
            "-frames:v",
            "1",
            "-q:v",
            "2",
            path.to_str().unwrap(),
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("placeholder avatar: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Ffmpeg("failed to create placeholder avatar".into()));
    }
    Ok(())
}

fn word_wrap(text: &str, max_chars: usize, max_lines: usize) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    let cleaned = cleaned.trim();

    let mut result = String::new();
    let mut lines = 0;

    for paragraph in cleaned.split('\n') {
        if lines >= max_lines {
            break;
        }
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }

        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.len() + word.len() + 1 > max_chars && !line.is_empty() {
                if lines >= max_lines - 1 {
                    if result.ends_with('\n') {
                        result.push_str(&line[..line.len().saturating_sub(3)]);
                        result.push_str("...");
                    } else {
                        let idx = line.len().min(max_chars.saturating_sub(3));
                        result.push_str(&line[..idx]);
                        result.push_str("...");
                    }
                    return result;
                }
                result.push_str(&line);
                result.push('\n');
                lines += 1;
                line = word.to_string();
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            if lines >= max_lines {
                break;
            }
            result.push_str(&line);
            result.push('\n');
            lines += 1;
        }
    }

    result.trim_end().to_string()
}

fn format_footer(tweet: &TweetRef) -> String {
    let likes = tweet
        .likes
        .map(|n| format_count(n))
        .unwrap_or_default();
    let ts = tweet.created_at.trim();
    if ts.is_empty() && likes.is_empty() {
        String::new()
    } else if ts.is_empty() {
        format!("{} Likes", likes)
    } else if likes.is_empty() {
        ts.to_string()
    } else {
        format!("{} · {} Likes", ts, likes)
    }
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn find_font() -> Option<PathBuf> {
    let candidates = ["fonts/Geist-Regular.ttf", "fonts/Geist-Regular.otf", "fonts/Geist.ttf"];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn escape_text_value(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('\'', "\\'")
     .replace(':', "\\:")
}
