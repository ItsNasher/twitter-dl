use bytes::Bytes;
use reqwest::Client;

use crate::error::AppError;

pub async fn fetch_and_convert_captions(
    client: &Client,
    vtt_url: &str,
) -> Result<Bytes, AppError> {
    let vtt = client
        .get(vtt_url)
        .send()
        .await?
        .text()
        .await?;

    let srt = vtt_to_srt(&vtt);
    Ok(Bytes::from(srt.into_bytes()))
}

fn vtt_to_srt(vtt: &str) -> String {
    let mut srt = String::new();
    let mut index = 1u32;

    let blocks: Vec<&str> = vtt
        .split("\n\n")
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .collect();

    for block in blocks {
        let lines: Vec<&str> = block.lines().collect();

        if lines.is_empty() { continue; }
        if lines[0].starts_with("WEBVTT") { continue; }
        if lines[0].starts_with("NOTE") { continue; }
        if lines[0].starts_with("STYLE") { continue; }
        if lines[0].starts_with("REGION") { continue; }

        let ts_line_idx = lines.iter().position(|l| l.contains("-->"));
        let ts_line_idx = match ts_line_idx {
            Some(i) => i,
            None => continue,
        };

        let ts_line = lines[ts_line_idx];

        let ts_srt = ts_line
            .replace('.', ",")
            .split_once("  ")
            .map(|(t, _)| t.to_string())
            .unwrap_or_else(|| ts_line.replace('.', ",").to_string());

        let text: Vec<&str> = lines[(ts_line_idx + 1)..].to_vec();
        if text.is_empty() { continue; }

        let clean_text = text
            .iter()
            .map(|l| strip_vtt_tags(l))
            .collect::<Vec<_>>()
            .join("\n");

        if clean_text.trim().is_empty() { continue; }

        srt.push_str(&format!("{}\n{}\n{}\n\n", index, ts_srt, clean_text));
        index += 1;
    }

    srt
}

fn strip_vtt_tags(line: &str) -> String {
    let re = regex::Regex::new(r"<[^>]+>").expect("valid regex");
    re.replace_all(line, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vtt_to_srt_basic() {
        let vtt = r#"WEBVTT

00:00:01.000 --> 00:00:03.000
Hello world

00:00:04.500 --> 00:00:06.000
Second line"#;

        let srt = vtt_to_srt(vtt);
        assert!(srt.contains("00:00:01,000 --> 00:00:03,000"));
        assert!(srt.contains("Hello world"));
        assert!(srt.contains("00:00:04,500 --> 00:00:06,000"));
    }

    #[test]
    fn test_strip_vtt_tags() {
        let line = "<c>Hello</c> <00:00:01.500><b>world</b>";
        assert_eq!(strip_vtt_tags(line), "Hello  world");
    }
}