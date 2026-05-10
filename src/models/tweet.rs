use serde::{Deserialize, Serialize};
use crate::models::VideoVariant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TweetRef {
    pub author: String,
    pub text: String,
    pub avatar_url: Option<String>,
    pub variants: Vec<VideoVariant>,
    pub likes: Option<u64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TweetInfo {
    pub author: String,
    pub created_at: String,
    pub text: String,
    pub quoted_tweet: Option<TweetRef>,
    pub in_reply_to: Option<TweetRef>,
    pub variants: Vec<VideoVariant>,
    pub avatar_url: Option<String>,
    pub likes: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct SyndicationTweet {
    #[serde(default)]
    pub user: SyndicationUser,
    #[serde(alias = "text", default)]
    pub full_text: String,
    #[serde(rename = "created_at", default)]
    pub created_at: String,
    #[serde(rename = "extended_entities")]
    pub extended_entities: Option<MediaEntities>,
    #[serde(rename = "mediaDetails")]
    pub media_details: Option<Vec<MediaItem>>,
    #[serde(rename = "entities")]
    pub entities: Option<EntityBlock>,
    #[serde(rename = "quoted_tweet")]
    pub quoted_tweet: Option<SyndicationQuotedTweet>,
    #[serde(rename = "in_reply_to_screen_name")]
    pub in_reply_to_screen_name: Option<String>,
    #[serde(rename = "in_reply_to_status_id_str")]
    pub in_reply_to_status_id_str: Option<String>,
    #[serde(default, alias = "favorite_count")]
    pub likes: Option<u64>,
}

impl SyndicationTweet {
    pub fn video_media(&self) -> Option<&[MediaItem]> {
        if let Some(ee) = &self.extended_entities {
            if !ee.media.is_empty() {
                return Some(&ee.media);
            }
        }
        if let Some(md) = &self.media_details {
            if !md.is_empty() {
                return Some(md);
            }
        }
        None
    }
}

#[derive(Debug, Deserialize)]
pub struct MediaEntities {
    pub media: Vec<MediaItem>,
}

#[derive(Debug, Deserialize)]
pub struct EntityBlock {
    #[serde(default)]
    pub media: Vec<serde_json::Value>,
}

pub type ExtendedEntities = MediaEntities;

#[derive(Debug, Deserialize, Default)]
pub struct SyndicationUser {
    pub screen_name: String,
    #[serde(default)]
    pub profile_image_url_https: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MediaItem {
    #[serde(rename = "type", alias = "mediaType", default)]
    pub media_type: String,
    pub video_info: Option<VideoInfo>,
}

#[derive(Debug, Deserialize)]
pub struct VideoInfo {
    pub variants: Vec<RawVariant>,
    pub subtitles: Option<Vec<SubtitleTrack>>,
}

#[derive(Debug, Deserialize)]
pub struct RawVariant {
    #[serde(rename = "content_type", alias = "contentType", default)]
    pub content_type: String,
    pub url: String,
    pub bitrate: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubtitleTrack {
    pub url: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct SyndicationQuotedTweet {
    pub user: SyndicationUser,
    #[serde(alias = "text", default)]
    pub full_text: String,
    #[serde(rename = "id_str", default)]
    pub id_str: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InfoRequest {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub include_quote: bool,
    #[serde(default)]
    pub include_reply: bool,
}