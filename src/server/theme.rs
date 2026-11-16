use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct ThemePalette {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_selection")]
    pub selection: String,
    #[serde(default = "default_muted")]
    pub muted: String,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_dark_background")]
    pub dark_background: String,
    #[serde(default = "default_darker_background")]
    pub darker_background: String,
    #[serde(default = "default_lighter_background")]
    pub lighter_background: String,
    #[serde(default = "default_foreground")]
    pub foreground: String,
    #[serde(default = "default_dark_foreground")]
    pub dark_foreground: String,
    #[serde(default = "default_bright_foreground")]
    pub bright_foreground: String,
    #[serde(default = "default_green")]
    pub green: String,
    #[serde(default = "default_red")]
    pub red: String,
    #[serde(default = "default_yellow")]
    pub yellow: String,
    #[serde(default = "default_blue")]
    pub blue: String,
    #[serde(default = "default_cyan")]
    pub cyan: String,
}

fn default_mode() -> String {
    "dark".into()
}
fn default_accent() -> String {
    "#7aa2f7".into()
}
fn default_selection() -> String {
    "#292e42".into()
}
fn default_muted() -> String {
    "#414868".into()
}
fn default_background() -> String {
    "#1a1b26".into()
}
fn default_dark_background() -> String {
    "#13141c".into()
}
fn default_darker_background() -> String {
    "#0e0e14".into()
}
fn default_lighter_background() -> String {
    "#24283b".into()
}
fn default_foreground() -> String {
    "#a9b1d6".into()
}
fn default_dark_foreground() -> String {
    "#565f89".into()
}
fn default_bright_foreground() -> String {
    "#c0caf5".into()
}
fn default_green() -> String {
    "#9ece6a".into()
}
fn default_red() -> String {
    "#f7768e".into()
}
fn default_yellow() -> String {
    "#e0af68".into()
}
fn default_blue() -> String {
    "#7aa2f7".into()
}
fn default_cyan() -> String {
    "#449dab".into()
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            accent: default_accent(),
            selection: default_selection(),
            muted: default_muted(),
            background: default_background(),
            dark_background: default_dark_background(),
            darker_background: default_darker_background(),
            lighter_background: default_lighter_background(),
            foreground: default_foreground(),
            dark_foreground: default_dark_foreground(),
            bright_foreground: default_bright_foreground(),
            green: default_green(),
            red: default_red(),
            yellow: default_yellow(),
            blue: default_blue(),
            cyan: default_cyan(),
        }
    }
}

impl ThemePalette {
    pub fn load_current() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let colors_path = home.join(".local/state/omarchy/current/theme/colors.toml");
        Self::load_from_path(&colors_path)
    }

    pub fn load_from_path(path: &Path) -> Self {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(palette) = toml::from_str::<ThemePalette>(&content) {
                return palette;
            }
        }
        Self::default()
    }

    pub fn generate_qr_svg(&self, url: &str) -> Result<String, String> {
        let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M)
            .map_err(|e| format!("Failed to generate QR code: {e}"))?;
        let dark = if self.bright_foreground.starts_with('#') {
            &self.bright_foreground
        } else {
            "#c0caf5"
        };
        let light = if self.darker_background.starts_with('#') {
            &self.darker_background
        } else {
            "#0e0e14"
        };

        let svg_image = code
            .render::<svg::Color>()
            .min_dimensions(240, 240)
            .dark_color(svg::Color(dark))
            .light_color(svg::Color(light))
            .build();
        Ok(svg_image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_theme_palette_default() {
        let palette = ThemePalette::default();
        assert_eq!(palette.mode, "dark");
        assert_eq!(palette.accent, "#7aa2f7");
        assert_eq!(palette.background, "#1a1b26");
        assert_eq!(palette.green, "#9ece6a");
    }

    #[test]
    fn test_theme_palette_load_from_path() {
        let temp_path =
            std::env::temp_dir().join(format!("colors_test_{}.toml", std::process::id()));
        let content = r##"
mode = "light"
accent = "#ff007f"
background = "#ffffff"
lighter_background = "#f0f0f0"
bright_foreground = "#111111"
"##;
        fs::write(&temp_path, content).unwrap();

        let palette = ThemePalette::load_from_path(&temp_path);
        let _ = fs::remove_file(&temp_path);

        assert_eq!(palette.mode, "light");
        assert_eq!(palette.accent, "#ff007f");
        assert_eq!(palette.background, "#ffffff");
        assert_eq!(palette.green, "#9ece6a"); // default preserved
    }

    #[test]
    fn test_theme_palette_missing_file_fallback() {
        let non_existent = Path::new("/non/existent/colors.toml");
        let palette = ThemePalette::load_from_path(non_existent);
        assert_eq!(palette, ThemePalette::default());
    }

    #[test]
    fn test_generate_qr_svg() {
        let palette = ThemePalette::default();
        let svg = palette
            .generate_qr_svg("http://192.168.1.100:9697")
            .unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));

        // QR code generation fails if payload exceeds max data capacity
        let huge_url = "a".repeat(10000);
        assert!(palette.generate_qr_svg(&huge_url).is_err());
    }
}
