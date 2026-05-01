use crate::error::EnhancedError;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub description: String,
    pub colors: ThemeColors,
    pub styles: ThemeStyles,
    pub is_high_contrast: bool,
    pub is_dark: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    // Background colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub background: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub surface: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub surface_variant: Color,

    // Text colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_background: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_surface: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_surface_variant: Color,

    // Primary colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub primary: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_primary: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub primary_container: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_primary_container: Color,

    // Secondary colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub secondary: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_secondary: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub secondary_container: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_secondary_container: Color,

    // Error colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub error: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_error: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub error_container: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub on_error_container: Color,

    // Outline colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub outline: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub outline_variant: Color,

    // Special colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub success: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub warning: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub info: Color,

    // Code syntax colors
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_keyword: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_string: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_number: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_comment: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_function: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_color"
    )]
    pub syntax_type: Color,
}

#[derive(Debug, Clone)]
pub struct ThemeStyles {
    pub normal: Style,
    pub bold: Style,
    pub italic: Style,
    pub underline: Style,
    pub strikethrough: Style,
    pub dim: Style,
    pub reversed: Style,
}

// Custom serialization for Color
fn serialize_color<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match color {
        Color::Rgb(r, g, b) => {
            let color_str = format!("rgb({},{},{})", r, g, b);
            serializer.serialize_str(&color_str)
        }
        Color::Reset => serializer.serialize_str("reset"),
        Color::Black => serializer.serialize_str("black"),
        Color::Red => serializer.serialize_str("red"),
        Color::Green => serializer.serialize_str("green"),
        Color::Yellow => serializer.serialize_str("yellow"),
        Color::Blue => serializer.serialize_str("blue"),
        Color::Magenta => serializer.serialize_str("magenta"),
        Color::Cyan => serializer.serialize_str("cyan"),
        Color::Gray => serializer.serialize_str("gray"),
        Color::DarkGray => serializer.serialize_str("dark_gray"),
        Color::LightRed => serializer.serialize_str("light_red"),
        Color::LightGreen => serializer.serialize_str("light_green"),
        Color::LightYellow => serializer.serialize_str("light_yellow"),
        Color::LightBlue => serializer.serialize_str("light_blue"),
        Color::LightMagenta => serializer.serialize_str("light_magenta"),
        Color::LightCyan => serializer.serialize_str("light_cyan"),
        Color::White => serializer.serialize_str("white"),
        Color::Indexed(i) => {
            let color_str = format!("indexed({})", i);
            serializer.serialize_str(&color_str)
        }
    }
}

// Custom deserialization for Color
fn deserialize_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.as_str() {
        "reset" => Ok(Color::Reset),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" => Ok(Color::Gray),
        "dark_gray" => Ok(Color::DarkGray),
        "light_red" => Ok(Color::LightRed),
        "light_green" => Ok(Color::LightGreen),
        "light_yellow" => Ok(Color::LightYellow),
        "light_blue" => Ok(Color::LightBlue),
        "light_magenta" => Ok(Color::LightMagenta),
        "light_cyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        _ => {
            if s.starts_with("rgb(") && s.ends_with(')') {
                let rgb_str = &s[4..s.len() - 1];
                let parts: Vec<&str> = rgb_str.split(',').collect();
                if parts.len() == 3 {
                    let r = parts[0].parse::<u8>().map_err(serde::de::Error::custom)?;
                    let g = parts[1].parse::<u8>().map_err(serde::de::Error::custom)?;
                    let b = parts[2].parse::<u8>().map_err(serde::de::Error::custom)?;
                    Ok(Color::Rgb(r, g, b))
                } else {
                    Err(serde::de::Error::custom("Invalid RGB format"))
                }
            } else if s.starts_with("indexed(") && s.ends_with(')') {
                let index_str = &s[8..s.len() - 1];
                let index = index_str.parse::<u8>().map_err(serde::de::Error::custom)?;
                Ok(Color::Indexed(index))
            } else {
                Err(serde::de::Error::custom(format!("Unknown color: {}", s)))
            }
        }
    }
}

// Manual implementation of Serialize and Deserialize for ThemeStyles
impl Serialize for ThemeStyles {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ThemeStyles", 1)?;
        state.serialize_field("type", "default")?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ThemeStyles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct ThemeStylesVisitor;

        impl<'de> Visitor<'de> for ThemeStylesVisitor {
            type Value = ThemeStyles;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a ThemeStyles object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<ThemeStyles, V::Error>
            where
                V: MapAccess<'de>,
            {
                // Just consume the map and return default styles
                while let Some(_) = map.next_entry::<String, String>()? {}
                Ok(ThemeStyles::create_default_styles())
            }
        }

        deserializer.deserialize_map(ThemeStylesVisitor)
    }
}

impl ThemeStyles {
    fn create_default_styles() -> Self {
        Self {
            normal: Style::default(),
            bold: Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            italic: Style::default().add_modifier(ratatui::style::Modifier::ITALIC),
            underline: Style::default().add_modifier(ratatui::style::Modifier::UNDERLINED),
            strikethrough: Style::default().add_modifier(ratatui::style::Modifier::CROSSED_OUT),
            dim: Style::default().add_modifier(ratatui::style::Modifier::DIM),
            reversed: Style::default().add_modifier(ratatui::style::Modifier::REVERSED),
        }
    }

    fn create_high_contrast_styles() -> Self {
        Self {
            normal: Style::default(),
            bold: Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            italic: Style::default().add_modifier(ratatui::style::Modifier::ITALIC),
            underline: Style::default()
                .add_modifier(ratatui::style::Modifier::UNDERLINED)
                .add_modifier(ratatui::style::Modifier::BOLD),
            strikethrough: Style::default()
                .add_modifier(ratatui::style::Modifier::CROSSED_OUT)
                .add_modifier(ratatui::style::Modifier::BOLD),
            dim: Style::default(), // No dim in high contrast
            reversed: Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
                .add_modifier(ratatui::style::Modifier::BOLD),
        }
    }
}

#[derive(Clone)]
pub struct ThemeService {
    themes: HashMap<String, Theme>,
    current_theme: String,
    custom_themes_path: Option<String>,
}

impl ThemeService {
    pub fn new() -> Self {
        let mut service = Self {
            themes: HashMap::new(),
            current_theme: "rust".to_string(),
            custom_themes_path: None,
        };

        service.load_builtin_themes();
        service
    }

    pub fn with_custom_themes_path<P: AsRef<Path>>(path: P) -> Self {
        let mut service = Self::new();
        service.custom_themes_path = Some(path.as_ref().to_string_lossy().to_string());
        service.load_custom_themes().ok(); // Ignore errors for missing custom themes
        service
    }

    fn load_builtin_themes(&mut self) {
        // Rust theme (default)
        self.themes
            .insert("rust".to_string(), Self::create_rust_theme());

        // Midnight theme (dark)
        self.themes
            .insert("midnight".to_string(), Self::create_midnight_theme());

        // Solar theme (light)
        self.themes
            .insert("solar".to_string(), Self::create_solar_theme());

        // High contrast themes
        self.themes.insert(
            "high_contrast_light".to_string(),
            Self::create_high_contrast_light_theme(),
        );
        self.themes.insert(
            "high_contrast_dark".to_string(),
            Self::create_high_contrast_dark_theme(),
        );
    }

    fn create_rust_theme() -> Theme {
        let colors = ThemeColors {
            background: Color::Rgb(40, 40, 40),
            surface: Color::Rgb(50, 50, 50),
            surface_variant: Color::Rgb(60, 60, 60),

            on_background: Color::Rgb(255, 255, 255),
            on_surface: Color::Rgb(240, 240, 240),
            on_surface_variant: Color::Rgb(200, 200, 200),

            primary: Color::Rgb(222, 165, 132),
            on_primary: Color::Rgb(40, 40, 40),
            primary_container: Color::Rgb(180, 120, 90),
            on_primary_container: Color::Rgb(255, 255, 255),

            secondary: Color::Rgb(134, 151, 142),
            on_secondary: Color::Rgb(40, 40, 40),
            secondary_container: Color::Rgb(100, 120, 110),
            on_secondary_container: Color::Rgb(255, 255, 255),

            error: Color::Rgb(231, 76, 60),
            on_error: Color::Rgb(255, 255, 255),
            error_container: Color::Rgb(200, 50, 40),
            on_error_container: Color::Rgb(255, 255, 255),

            outline: Color::Rgb(120, 120, 120),
            outline_variant: Color::Rgb(80, 80, 80),

            success: Color::Rgb(46, 204, 113),
            warning: Color::Rgb(241, 196, 15),
            info: Color::Rgb(52, 152, 219),

            syntax_keyword: Color::Rgb(86, 156, 214),
            syntax_string: Color::Rgb(206, 145, 120),
            syntax_number: Color::Rgb(181, 206, 168),
            syntax_comment: Color::Rgb(106, 153, 85),
            syntax_function: Color::Rgb(220, 220, 170),
            syntax_type: Color::Rgb(78, 201, 176),
        };

        Theme {
            name: "Rust".to_string(),
            description: "A warm, Rust-inspired theme with orange and brown tones".to_string(),
            colors,
            styles: ThemeStyles::create_default_styles(),
            is_high_contrast: false,
            is_dark: true,
        }
    }

    fn create_midnight_theme() -> Theme {
        let colors = ThemeColors {
            background: Color::Rgb(13, 17, 23),
            surface: Color::Rgb(22, 27, 34),
            surface_variant: Color::Rgb(33, 38, 45),

            on_background: Color::Rgb(240, 246, 252),
            on_surface: Color::Rgb(201, 209, 217),
            on_surface_variant: Color::Rgb(139, 148, 158),

            primary: Color::Rgb(88, 166, 255),
            on_primary: Color::Rgb(13, 17, 23),
            primary_container: Color::Rgb(0, 92, 197),
            on_primary_container: Color::Rgb(240, 246, 252),

            secondary: Color::Rgb(125, 196, 228),
            on_secondary: Color::Rgb(13, 17, 23),
            secondary_container: Color::Rgb(0, 120, 160),
            on_secondary_container: Color::Rgb(240, 246, 252),

            error: Color::Rgb(248, 81, 73),
            on_error: Color::Rgb(255, 255, 255),
            error_container: Color::Rgb(164, 14, 38),
            on_error_container: Color::Rgb(255, 255, 255),

            outline: Color::Rgb(110, 118, 129),
            outline_variant: Color::Rgb(68, 76, 86),

            success: Color::Rgb(63, 185, 80),
            warning: Color::Rgb(255, 212, 59),
            info: Color::Rgb(88, 166, 255),

            syntax_keyword: Color::Rgb(255, 123, 114),
            syntax_string: Color::Rgb(165, 214, 167),
            syntax_number: Color::Rgb(121, 192, 255),
            syntax_comment: Color::Rgb(139, 148, 158),
            syntax_function: Color::Rgb(210, 168, 255),
            syntax_type: Color::Rgb(255, 184, 108),
        };

        Theme {
            name: "Midnight".to_string(),
            description: "A dark theme inspired by GitHub's dark mode".to_string(),
            colors,
            styles: ThemeStyles::create_default_styles(),
            is_high_contrast: false,
            is_dark: true,
        }
    }

    fn create_solar_theme() -> Theme {
        let colors = ThemeColors {
            background: Color::Rgb(253, 246, 227),
            surface: Color::Rgb(238, 232, 213),
            surface_variant: Color::Rgb(220, 215, 200),

            on_background: Color::Rgb(101, 123, 131),
            on_surface: Color::Rgb(88, 110, 117),
            on_surface_variant: Color::Rgb(147, 161, 161),

            primary: Color::Rgb(38, 139, 210),
            on_primary: Color::Rgb(253, 246, 227),
            primary_container: Color::Rgb(7, 54, 66),
            on_primary_container: Color::Rgb(253, 246, 227),

            secondary: Color::Rgb(42, 161, 152),
            on_secondary: Color::Rgb(253, 246, 227),
            secondary_container: Color::Rgb(0, 43, 54),
            on_secondary_container: Color::Rgb(253, 246, 227),

            error: Color::Rgb(220, 50, 47),
            on_error: Color::Rgb(253, 246, 227),
            error_container: Color::Rgb(203, 75, 22),
            on_error_container: Color::Rgb(253, 246, 227),

            outline: Color::Rgb(147, 161, 161),
            outline_variant: Color::Rgb(181, 137, 0),

            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(181, 137, 0),
            info: Color::Rgb(38, 139, 210),

            syntax_keyword: Color::Rgb(181, 137, 0),
            syntax_string: Color::Rgb(42, 161, 152),
            syntax_number: Color::Rgb(220, 50, 47),
            syntax_comment: Color::Rgb(147, 161, 161),
            syntax_function: Color::Rgb(38, 139, 210),
            syntax_type: Color::Rgb(108, 113, 196),
        };

        Theme {
            name: "Solar".to_string(),
            description: "A light theme inspired by Solarized Light".to_string(),
            colors,
            styles: ThemeStyles::create_default_styles(),
            is_high_contrast: false,
            is_dark: false,
        }
    }

    fn create_high_contrast_light_theme() -> Theme {
        let colors = ThemeColors {
            background: Color::Rgb(255, 255, 255),
            surface: Color::Rgb(248, 248, 248),
            surface_variant: Color::Rgb(240, 240, 240),

            on_background: Color::Rgb(0, 0, 0),
            on_surface: Color::Rgb(0, 0, 0),
            on_surface_variant: Color::Rgb(0, 0, 0),

            primary: Color::Rgb(0, 0, 255),
            on_primary: Color::Rgb(255, 255, 255),
            primary_container: Color::Rgb(0, 0, 200),
            on_primary_container: Color::Rgb(255, 255, 255),

            secondary: Color::Rgb(0, 128, 0),
            on_secondary: Color::Rgb(255, 255, 255),
            secondary_container: Color::Rgb(0, 100, 0),
            on_secondary_container: Color::Rgb(255, 255, 255),

            error: Color::Rgb(255, 0, 0),
            on_error: Color::Rgb(255, 255, 255),
            error_container: Color::Rgb(200, 0, 0),
            on_error_container: Color::Rgb(255, 255, 255),

            outline: Color::Rgb(0, 0, 0),
            outline_variant: Color::Rgb(100, 100, 100),

            success: Color::Rgb(0, 128, 0),
            warning: Color::Rgb(255, 165, 0),
            info: Color::Rgb(0, 0, 255),

            syntax_keyword: Color::Rgb(0, 0, 255),
            syntax_string: Color::Rgb(0, 128, 0),
            syntax_number: Color::Rgb(255, 0, 0),
            syntax_comment: Color::Rgb(128, 128, 128),
            syntax_function: Color::Rgb(128, 0, 128),
            syntax_type: Color::Rgb(0, 128, 128),
        };

        Theme {
            name: "High Contrast Light".to_string(),
            description: "WCAG AAA compliant high contrast light theme".to_string(),
            colors,
            styles: ThemeStyles::create_high_contrast_styles(),
            is_high_contrast: true,
            is_dark: false,
        }
    }

    fn create_high_contrast_dark_theme() -> Theme {
        let colors = ThemeColors {
            background: Color::Rgb(0, 0, 0),
            surface: Color::Rgb(16, 16, 16),
            surface_variant: Color::Rgb(32, 32, 32),

            on_background: Color::Rgb(255, 255, 255),
            on_surface: Color::Rgb(255, 255, 255),
            on_surface_variant: Color::Rgb(255, 255, 255),

            primary: Color::Rgb(0, 255, 255),
            on_primary: Color::Rgb(0, 0, 0),
            primary_container: Color::Rgb(0, 200, 200),
            on_primary_container: Color::Rgb(0, 0, 0),

            secondary: Color::Rgb(255, 255, 0),
            on_secondary: Color::Rgb(0, 0, 0),
            secondary_container: Color::Rgb(200, 200, 0),
            on_secondary_container: Color::Rgb(0, 0, 0),

            error: Color::Rgb(255, 100, 100),
            on_error: Color::Rgb(0, 0, 0),
            error_container: Color::Rgb(255, 50, 50),
            on_error_container: Color::Rgb(0, 0, 0),

            outline: Color::Rgb(255, 255, 255),
            outline_variant: Color::Rgb(200, 200, 200),

            success: Color::Rgb(0, 255, 0),
            warning: Color::Rgb(255, 255, 0),
            info: Color::Rgb(0, 255, 255),

            syntax_keyword: Color::Rgb(0, 255, 255),
            syntax_string: Color::Rgb(0, 255, 0),
            syntax_number: Color::Rgb(255, 100, 100),
            syntax_comment: Color::Rgb(200, 200, 200),
            syntax_function: Color::Rgb(255, 0, 255),
            syntax_type: Color::Rgb(255, 255, 0),
        };

        Theme {
            name: "High Contrast Dark".to_string(),
            description: "WCAG AAA compliant high contrast dark theme".to_string(),
            colors,
            styles: ThemeStyles::create_high_contrast_styles(),
            is_high_contrast: true,
            is_dark: true,
        }
    }

    #[allow(dead_code)] // Future functionality
    fn create_default_styles() -> ThemeStyles {
        ThemeStyles {
            normal: Style::default(),
            bold: Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            italic: Style::default().add_modifier(ratatui::style::Modifier::ITALIC),
            underline: Style::default().add_modifier(ratatui::style::Modifier::UNDERLINED),
            strikethrough: Style::default().add_modifier(ratatui::style::Modifier::CROSSED_OUT),
            dim: Style::default().add_modifier(ratatui::style::Modifier::DIM),
            reversed: Style::default().add_modifier(ratatui::style::Modifier::REVERSED),
        }
    }

    #[allow(dead_code)] // Future functionality
    fn create_high_contrast_styles() -> ThemeStyles {
        ThemeStyles {
            normal: Style::default(),
            bold: Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            italic: Style::default().add_modifier(ratatui::style::Modifier::ITALIC),
            underline: Style::default()
                .add_modifier(ratatui::style::Modifier::UNDERLINED)
                .add_modifier(ratatui::style::Modifier::BOLD),
            strikethrough: Style::default()
                .add_modifier(ratatui::style::Modifier::CROSSED_OUT)
                .add_modifier(ratatui::style::Modifier::BOLD),
            dim: Style::default(), // No dim in high contrast
            reversed: Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
                .add_modifier(ratatui::style::Modifier::BOLD),
        }
    }

    pub fn get_available_themes(&self) -> Vec<&Theme> {
        self.themes.values().collect()
    }

    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    pub fn get_current_theme(&self) -> &Theme {
        self.themes
            .get(&self.current_theme)
            .expect("Current theme should always exist")
    }

    pub fn set_theme(&mut self, name: &str) -> Result<(), EnhancedError> {
        if !self.themes.contains_key(name) {
            return Err(EnhancedError::ui(format!("Theme not found: {}", name)));
        }

        self.current_theme = name.to_string();
        Ok(())
    }

    /// Toggle between light and dark themes
    pub fn toggle_theme(&mut self) -> Result<(), EnhancedError> {
        let current_theme = self.get_current_theme();
        let new_theme_name = if current_theme.is_dark {
            "solar" // Light theme
        } else {
            "rust" // Dark theme
        };
        self.set_theme(new_theme_name)
    }

    pub fn add_custom_theme(&mut self, theme: Theme) -> Result<(), EnhancedError> {
        if self.themes.contains_key(&theme.name) {
            return Err(EnhancedError::ui(format!(
                "Theme already exists: {}",
                theme.name
            )));
        }

        self.themes.insert(theme.name.clone(), theme);
        Ok(())
    }

    pub fn save_custom_theme(&self, theme: &Theme) -> Result<(), EnhancedError> {
        let custom_path = self
            .custom_themes_path
            .as_ref()
            .ok_or_else(|| EnhancedError::ui("No custom themes path configured"))?;

        let theme_file = format!("{}/{}.json", custom_path, theme.name);
        let theme_json = serde_json::to_string_pretty(theme)
            .map_err(|e| EnhancedError::ui(format!("Failed to serialize theme: {}", e)))?;

        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(&theme_file).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EnhancedError::ui(format!("Failed to create theme directory: {}", e))
            })?;
        }

        fs::write(&theme_file, theme_json)
            .map_err(|e| EnhancedError::ui(format!("Failed to save theme file: {}", e)))?;

        Ok(())
    }

    pub fn load_custom_themes(&mut self) -> Result<(), EnhancedError> {
        let custom_path = self
            .custom_themes_path
            .as_ref()
            .ok_or_else(|| EnhancedError::ui("No custom themes path configured"))?;

        if !Path::new(custom_path).exists() {
            return Ok(()); // No custom themes directory, that's fine
        }

        let entries = fs::read_dir(custom_path).map_err(|e| {
            EnhancedError::ui(format!("Failed to read custom themes directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry
                .map_err(|e| EnhancedError::ui(format!("Failed to read directory entry: {}", e)))?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_theme_from_file(&path) {
                    Ok(theme) => {
                        self.themes.insert(theme.name.clone(), theme);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load theme from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    fn load_theme_from_file(&self, path: &Path) -> Result<Theme, EnhancedError> {
        let content = fs::read_to_string(path)
            .map_err(|e| EnhancedError::ui(format!("Failed to read theme file: {}", e)))?;

        let theme: Theme = serde_json::from_str(&content)
            .map_err(|e| EnhancedError::ui(format!("Failed to parse theme file: {}", e)))?;

        Ok(theme)
    }

    pub fn export_theme(&self, theme_name: &str, path: &Path) -> Result<(), EnhancedError> {
        let theme = self
            .get_theme(theme_name)
            .ok_or_else(|| EnhancedError::ui("Theme not found"))?;

        let theme_json = serde_json::to_string_pretty(theme)
            .map_err(|e| EnhancedError::ui(format!("Failed to serialize theme: {}", e)))?;

        fs::write(path, theme_json)
            .map_err(|e| EnhancedError::ui(format!("Failed to write theme file: {}", e)))?;

        Ok(())
    }

    pub fn import_theme(&mut self, path: &Path) -> Result<String, EnhancedError> {
        let theme = self.load_theme_from_file(path)?;
        let theme_name = theme.name.clone();

        self.themes.insert(theme_name.clone(), theme);
        Ok(theme_name)
    }

    pub fn validate_theme(&self, theme: &Theme) -> Result<(), EnhancedError> {
        // Basic validation
        if theme.name.is_empty() {
            return Err(EnhancedError::ui("Theme name cannot be empty"));
        }

        // Validate contrast ratios for accessibility
        if theme.is_high_contrast {
            self.validate_contrast_ratios(theme)?;
        }

        Ok(())
    }

    fn validate_contrast_ratios(&self, theme: &Theme) -> Result<(), EnhancedError> {
        // This is a simplified contrast validation
        // In a real implementation, you'd calculate actual contrast ratios
        let colors = &theme.colors;

        // Check that text colors have sufficient contrast with backgrounds
        if Self::colors_too_similar(colors.on_background, colors.background) {
            return Err(EnhancedError::ui(
                "Insufficient contrast between text and background",
            ));
        }

        if Self::colors_too_similar(colors.on_surface, colors.surface) {
            return Err(EnhancedError::ui(
                "Insufficient contrast between text and surface",
            ));
        }

        Ok(())
    }

    fn colors_too_similar(color1: Color, color2: Color) -> bool {
        // Simplified color similarity check
        // In a real implementation, you'd calculate proper contrast ratios
        match (color1, color2) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                let diff = ((r1 as i32 - r2 as i32).abs()
                    + (g1 as i32 - g2 as i32).abs()
                    + (b1 as i32 - b2 as i32).abs()) as f32;
                diff < 300.0 // Arbitrary threshold for demo
            }
            _ => false,
        }
    }
}

impl Default for ThemeService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_theme_service_creation() {
        let service = ThemeService::new();
        assert_eq!(service.current_theme, "rust");
        assert!(service.themes.len() >= 5); // At least 5 built-in themes
    }

    #[test]
    fn test_get_current_theme() {
        let service = ThemeService::new();
        let theme = service.get_current_theme();
        assert_eq!(theme.name, "Rust");
    }

    #[test]
    fn test_set_theme() {
        let mut service = ThemeService::new();

        // Test valid theme
        assert!(service.set_theme("midnight").is_ok());
        assert_eq!(service.current_theme, "midnight");

        // Test invalid theme
        assert!(service.set_theme("nonexistent").is_err());
    }

    #[test]
    fn test_get_available_themes() {
        let service = ThemeService::new();
        let themes = service.get_available_themes();
        assert!(themes.len() >= 5);

        let theme_names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        assert!(theme_names.contains(&"Rust"));
        assert!(theme_names.contains(&"Midnight"));
        assert!(theme_names.contains(&"Solar"));
        assert!(theme_names.contains(&"High Contrast Light"));
        assert!(theme_names.contains(&"High Contrast Dark"));
    }

    #[test]
    fn test_high_contrast_themes() {
        let service = ThemeService::new();

        let hc_light = service.get_theme("high_contrast_light").unwrap();
        assert!(hc_light.is_high_contrast);
        assert!(!hc_light.is_dark);

        let hc_dark = service.get_theme("high_contrast_dark").unwrap();
        assert!(hc_dark.is_high_contrast);
        assert!(hc_dark.is_dark);
    }

    #[test]
    fn test_custom_theme_operations() {
        let temp_dir = TempDir::new().unwrap();
        let mut service = ThemeService::with_custom_themes_path(temp_dir.path());

        // Create a custom theme
        let mut custom_theme = service.get_theme("rust").unwrap().clone();
        custom_theme.name = "Custom Test".to_string();
        custom_theme.description = "A test theme".to_string();

        // Add custom theme
        assert!(service.add_custom_theme(custom_theme.clone()).is_ok());
        assert!(service.get_theme("Custom Test").is_some());

        // Save custom theme
        assert!(service.save_custom_theme(&custom_theme).is_ok());

        // Load custom themes
        let new_service = ThemeService::with_custom_themes_path(temp_dir.path());
        assert!(new_service.get_theme("Custom Test").is_some());
    }

    #[test]
    fn test_theme_validation() {
        let service = ThemeService::new();
        let theme = service.get_theme("rust").unwrap();

        // Valid theme should pass
        assert!(service.validate_theme(theme).is_ok());

        // Invalid theme (empty name) should fail
        let mut invalid_theme = theme.clone();
        invalid_theme.name = String::new();
        assert!(service.validate_theme(&invalid_theme).is_err());
    }

    #[test]
    fn test_theme_export_import() {
        let temp_dir = TempDir::new().unwrap();
        let service = ThemeService::new();

        let export_path = temp_dir.path().join("test_theme.json");

        // Export theme
        assert!(service.export_theme("rust", &export_path).is_ok());
        assert!(export_path.exists());

        // Import theme
        let mut new_service = ThemeService::new();
        let imported_name = new_service.import_theme(&export_path).unwrap();
        assert_eq!(imported_name, "Rust");
    }
}
