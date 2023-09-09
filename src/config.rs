use std::path::PathBuf;

use home::home_dir;
use ratatui::{style::Color, widgets::BorderType};
use serde::{
    de::{DeserializeOwned, Visitor},
    Deserialize,
};
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone)]
pub struct VersesConfig {
    pub api: ApiConfiguration,
    pub general: GeneralConfiguration,
    pub theme: ThemeConfiguration,
}

impl VersesConfig {
    pub async fn read_from_str(str: &str) -> anyhow::Result<Self> {
        let unresolved = toml::from_str::<VersesConfigUnresolved>(str)?;
        let theme = unresolved.theme.resolve().await?;
        let api = unresolved.api.resolve().await?;
        let general = unresolved.general.resolve().await?;
        Ok(Self {
            theme,
            api,
            general,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeConfiguration {
    pub lyrics: ThemeLyrics,
    pub borders: ThemeBorders,
    pub progress_bar: ThemeProgress,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeLyrics {
    pub inactive_text_color: ThemeColor,
    pub active_text_color: ThemeColor,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeBorders {
    pub lyrics_border_color: ThemeColor,
    pub lyrics_border_text_color: ThemeColor,
    pub lyrics_border_style: BorderStyle,
    pub info_border_color: ThemeColor,
    pub info_border_text_color: ThemeColor,
    pub info_text_color: ThemeColor,
    pub info_border_style: BorderStyle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeProgress {
    pub color: ThemeColor,
    pub is_percentage: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfiguration {
    pub spotify_client_id: String,
    pub lyricstify_api_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralConfiguration {
    pub romanize_unicode: bool,
    pub romanize_exclude: Vec<String>,
    pub romanize_track_names: bool,
    pub scroll_offset: u32,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisplayConfig {
    pub show_name: bool,
    pub name_format: String,
    pub show_artists: bool,
    pub artists_format: String,
    pub show_album: bool,
    pub album_format: String,
    pub show_genres: bool,
    pub genres_format: String,
    pub show_popularity: bool,
    pub popularity_format: String,
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct ThemeColor(pub Color);

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        de.deserialize_str(ColorVisitor)
    }
}

struct ColorVisitor;

impl<'v> Visitor<'v> for ColorVisitor {
    type Value = ThemeColor;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a string representing a color")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if let Some(stripped) = v.strip_prefix('#') {
            let color_rgb = u32::from_str_radix(stripped, 16).map_err(|e| {
                serde::de::Error::custom(format!("Invalid hex string for color {e}"))
            })?;
            let r = (color_rgb & 0xFF0000) >> 16;
            let g = (color_rgb & 0x00FF00) >> 8;
            let b = color_rgb * 0x0000FF;
            Ok(ThemeColor(Color::Rgb(r as u8, g as u8, b as u8)))
        } else {
            v.parse::<Color>()
                .map_err(|e| serde::de::Error::custom(format!("Invalid named color format {e}")))
                .map(ThemeColor)
        }
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct BorderStyle(pub BorderType);

impl<'de> Deserialize<'de> for BorderStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(BorderVisitor).map(BorderStyle)
    }
}

struct BorderVisitor;

impl<'v> Visitor<'v> for BorderVisitor {
    type Value = BorderType;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a string representing a border color")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse::<BorderType>()
            .map_err(|e| serde::de::Error::custom(format!("Invalid border style: {e}")))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct VersesConfigUnresolved {
    general: MaybeLink<GeneralConfiguration>,
    api: MaybeLink<ApiConfiguration>,
    theme: MaybeLink<ThemeConfiguration>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum MaybeLink<V> {
    Explicit(V),
    Link { include: PathBuf },
}

impl<V: DeserializeOwned> MaybeLink<V> {
    async fn resolve(self) -> anyhow::Result<V> {
        match self {
            MaybeLink::Explicit(value) => Ok(value),
            MaybeLink::Link { include } => {
                let config_dir = home_dir().unwrap().join(".config").join("verses");
                let mut file = tokio::fs::File::open(config_dir.join(include)).await?;
                let mut str = String::new();
                file.read_to_string(&mut str).await?;
                toml::from_str(&str).map_err(anyhow::Error::from)
            }
        }
    }
}
