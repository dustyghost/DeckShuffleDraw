use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Key};
use serde::{Deserialize, Serialize};

pub const SETTINGS_FILE_NAME: &str = "settings.toml";
pub const DEFAULT_SETTINGS_FILE_NAME: &str = "settings.default.toml";

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub decks: Vec<DeckConfig>,
    pub active_deck: usize,
    pub card_max_width: f32,
    pub card_max_height: f32,
    pub help_max_width: f32,
    pub help_max_height: f32,
    pub card_corner_radius: f32,
    pub preload_cards: bool,
    pub keybindings: KeyBindings,
    pub ui_style: UiStyle,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppSettingsFile {
    image_dir: Option<PathBuf>,
    decks: Option<Vec<DeckConfigFile>>,
    active_deck: Option<usize>,
    card_max_width: f32,
    card_max_height: f32,
    help_max_width: f32,
    help_max_height: f32,
    card_corner_radius: f32,
    preload_cards: bool,
    keybindings: Option<KeyBindingsFile>,
    ui_style: Option<UiStyleFile>,
}

#[derive(Clone, Debug)]
pub struct DeckConfig {
    pub name: String,
    pub image_dir: PathBuf,
    pub help_image: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DeckConfigFile {
    name: Option<String>,
    image_dir: PathBuf,
    help_image: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBindings {
    pub next_card: Key,
    pub toggle_help: Key,
    pub toggle_deck_menu: Key,
    pub toggle_full_screen: Key,
    pub next_deck: Key,
    pub toggle_debug: Key,
    pub open_settings: Key,
    pub quit: Key,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyBindingsFile {
    next_card: String,
    toggle_help: String,
    toggle_deck_menu: String,
    #[serde(default = "default_toggle_full_screen_key")]
    toggle_full_screen: String,
    next_deck: String,
    toggle_debug: String,
    open_settings: String,
    quit: String,
}

#[derive(Clone, Debug)]
pub struct UiStyle {
    pub toolbar_fill: egui::Color32,
    pub toolbar_stroke: egui::Color32,
    pub next_card: egui::Color32,
    pub help: egui::Color32,
    pub deck_menu: egui::Color32,
    pub next_deck: egui::Color32,
    pub debug: egui::Color32,
    pub settings: egui::Color32,
    pub quit: egui::Color32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UiStyleFile {
    toolbar_fill: String,
    toolbar_stroke: String,
    next_card: String,
    help: String,
    deck_menu: String,
    next_deck: String,
    debug: String,
    settings: String,
    quit: String,
}

pub trait KeyDisplayExt {
    fn label(self) -> &'static str;
}

impl AppSettings {
    pub fn from_file(raw: AppSettingsFile, base_dir: &Path) -> Result<Self, String> {
        if raw.card_max_width <= 0.0 || raw.card_max_height <= 0.0 {
            return Err("Card size values must be greater than zero.".to_string());
        }
        if raw.help_max_width <= 0.0 || raw.help_max_height <= 0.0 {
            return Err("Help size values must be greater than zero.".to_string());
        }
        if raw.card_corner_radius < 0.0 {
            return Err("Card corner radius cannot be negative.".to_string());
        }

        let decks = build_decks(&raw, base_dir)?;
        let active_deck = raw.active_deck.unwrap_or(0).min(decks.len().saturating_sub(1));
        let keybindings = KeyBindings::from_file(raw.keybindings.unwrap_or_default())?;
        keybindings.validate_unique()?;
        let ui_style = UiStyle::from_file(raw.ui_style.unwrap_or_default())?;

        Ok(Self {
            decks,
            active_deck,
            card_max_width: raw.card_max_width,
            card_max_height: raw.card_max_height,
            help_max_width: raw.help_max_width,
            help_max_height: raw.help_max_height,
            card_corner_radius: raw.card_corner_radius,
            preload_cards: raw.preload_cards,
            keybindings,
            ui_style,
        })
    }

    pub fn to_file(&self, base_dir: &Path) -> AppSettingsFile {
        AppSettingsFile {
            image_dir: None,
            decks: Some(
                self.decks
                    .iter()
                    .map(|deck| DeckConfigFile {
                        name: Some(deck.name.clone()),
                        image_dir: relative_to(base_dir, &deck.image_dir),
                        help_image: deck.help_image.as_ref().map(|path| relative_to(base_dir, path)),
                    })
                    .collect(),
            ),
            active_deck: Some(self.active_deck),
            card_max_width: self.card_max_width,
            card_max_height: self.card_max_height,
            help_max_width: self.help_max_width,
            help_max_height: self.help_max_height,
            card_corner_radius: self.card_corner_radius,
            preload_cards: self.preload_cards,
            keybindings: Some(self.keybindings.to_file()),
            ui_style: Some(self.ui_style.to_file()),
        }
    }

    pub fn current_deck(&self) -> &DeckConfig {
        &self.decks[self.active_deck]
    }

    pub fn advance_deck(&mut self) {
        self.active_deck = (self.active_deck + 1) % self.decks.len();
    }
}

impl Default for KeyBindingsFile {
    fn default() -> Self {
        Self {
            next_card: "Space".to_string(),
            toggle_help: "H".to_string(),
            toggle_deck_menu: "M".to_string(),
            toggle_full_screen: "F".to_string(),
            next_deck: "Tab".to_string(),
            toggle_debug: "D".to_string(),
            open_settings: "S".to_string(),
            quit: "Q".to_string(),
        }
    }
}

fn default_toggle_full_screen_key() -> String {
    "F".to_string()
}

impl Default for UiStyleFile {
    fn default() -> Self {
        Self {
            toolbar_fill: "#22252c".to_string(),
            toolbar_stroke: "#404652".to_string(),
            next_card: "#2c7a5e".to_string(),
            help: "#3768b4".to_string(),
            deck_menu: "#6c4ea8".to_string(),
            next_deck: "#b27524".to_string(),
            debug: "#945034".to_string(),
            settings: "#5c6076".to_string(),
            quit: "#963040".to_string(),
        }
    }
}

impl KeyBindings {
    pub fn from_file(raw: KeyBindingsFile) -> Result<Self, String> {
        Ok(Self {
            next_card: parse_key(&raw.next_card)?,
            toggle_help: parse_key(&raw.toggle_help)?,
            toggle_deck_menu: parse_key(&raw.toggle_deck_menu)?,
            toggle_full_screen: parse_key(&raw.toggle_full_screen)?,
            next_deck: parse_key(&raw.next_deck)?,
            toggle_debug: parse_key(&raw.toggle_debug)?,
            open_settings: parse_key(&raw.open_settings)?,
            quit: parse_key(&raw.quit)?,
        })
    }

    pub fn to_file(&self) -> KeyBindingsFile {
        KeyBindingsFile {
            next_card: self.next_card.label().to_string(),
            toggle_help: self.toggle_help.label().to_string(),
            toggle_deck_menu: self.toggle_deck_menu.label().to_string(),
            toggle_full_screen: self.toggle_full_screen.label().to_string(),
            next_deck: self.next_deck.label().to_string(),
            toggle_debug: self.toggle_debug.label().to_string(),
            open_settings: self.open_settings.label().to_string(),
            quit: self.quit.label().to_string(),
        }
    }

    pub fn validate_unique(&self) -> Result<(), String> {
        let bindings = [
            ("next_card", self.next_card),
            ("toggle_help", self.toggle_help),
            ("toggle_deck_menu", self.toggle_deck_menu),
            ("toggle_full_screen", self.toggle_full_screen),
            ("next_deck", self.next_deck),
            ("toggle_debug", self.toggle_debug),
            ("open_settings", self.open_settings),
            ("quit", self.quit),
        ];
        let mut seen = HashSet::new();
        for (name, key) in bindings {
            if !seen.insert(key) {
                return Err(format!(
                    "Duplicate key binding detected for `{name}` on key `{}`.",
                    key.label()
                ));
            }
        }
        Ok(())
    }
}

impl UiStyle {
    pub fn from_file(raw: UiStyleFile) -> Result<Self, String> {
        Ok(Self {
            toolbar_fill: parse_color(&raw.toolbar_fill)?,
            toolbar_stroke: parse_color(&raw.toolbar_stroke)?,
            next_card: parse_color(&raw.next_card)?,
            help: parse_color(&raw.help)?,
            deck_menu: parse_color(&raw.deck_menu)?,
            next_deck: parse_color(&raw.next_deck)?,
            debug: parse_color(&raw.debug)?,
            settings: parse_color(&raw.settings)?,
            quit: parse_color(&raw.quit)?,
        })
    }

    pub fn to_file(&self) -> UiStyleFile {
        UiStyleFile {
            toolbar_fill: color_to_hex(self.toolbar_fill),
            toolbar_stroke: color_to_hex(self.toolbar_stroke),
            next_card: color_to_hex(self.next_card),
            help: color_to_hex(self.help),
            deck_menu: color_to_hex(self.deck_menu),
            next_deck: color_to_hex(self.next_deck),
            debug: color_to_hex(self.debug),
            settings: color_to_hex(self.settings),
            quit: color_to_hex(self.quit),
        }
    }
}

impl KeyDisplayExt for Key {
    fn label(self) -> &'static str {
        match self {
            Key::Space => "Space",
            Key::Tab => "Tab",
            Key::Enter => "Enter",
            Key::A => "A",
            Key::B => "B",
            Key::C => "C",
            Key::D => "D",
            Key::E => "E",
            Key::F => "F",
            Key::G => "G",
            Key::H => "H",
            Key::I => "I",
            Key::J => "J",
            Key::K => "K",
            Key::L => "L",
            Key::M => "M",
            Key::N => "N",
            Key::O => "O",
            Key::P => "P",
            Key::Q => "Q",
            Key::R => "R",
            Key::S => "S",
            Key::T => "T",
            Key::U => "U",
            Key::V => "V",
            Key::W => "W",
            Key::X => "X",
            Key::Y => "Y",
            Key::Z => "Z",
            _ => "Unknown",
        }
    }
}

pub fn default_app_settings() -> AppSettings {
    AppSettings {
        decks: vec![DeckConfig {
            name: "Deck 1".to_string(),
            image_dir: PathBuf::new(),
            help_image: None,
        }],
        active_deck: 0,
        card_max_width: 420.0,
        card_max_height: 640.0,
        help_max_width: 900.0,
        help_max_height: 1100.0,
        card_corner_radius: 16.0,
        preload_cards: true,
        keybindings: KeyBindings::from_file(KeyBindingsFile::default())
            .expect("default keybindings should be valid"),
        ui_style: UiStyle::from_file(UiStyleFile::default())
            .expect("default ui style should be valid"),
    }
}

pub fn load_settings() -> Result<(AppSettings, PathBuf), String> {
    let search_roots = [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf)),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
    ];
    load_settings_from_roots(search_roots)
}

fn load_settings_from_roots<I>(search_roots: I) -> Result<(AppSettings, PathBuf), String>
where
    I: IntoIterator<Item = Option<PathBuf>>,
{
    let search_roots = search_roots.into_iter();

    for root in search_roots.into_iter().flatten() {
        let settings_path = root.join(SETTINGS_FILE_NAME);
        if settings_path.is_file() {
            return read_settings_file(&settings_path, &root);
        }

        let default_settings_path = root.join(DEFAULT_SETTINGS_FILE_NAME);
        if default_settings_path.is_file() {
            let default_settings_text = fs::read_to_string(&default_settings_path).map_err(|error| {
                format!("Failed to read {}: {error}", default_settings_path.display())
            })?;
            fs::write(&settings_path, &default_settings_text).map_err(|error| {
                format!(
                    "Failed to create {} from {}: {error}",
                    settings_path.display(),
                    default_settings_path.display()
                )
            })?;
            return read_settings_file(&settings_path, &root);
        }
    }

    Err(format!("Could not find `{DEFAULT_SETTINGS_FILE_NAME}`."))
}

pub fn bindable_keys() -> &'static [Key] {
    &[
        Key::Space,
        Key::Tab,
        Key::Enter,
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ]
}

fn read_settings_file(settings_path: &Path, base_dir: &Path) -> Result<(AppSettings, PathBuf), String> {
    let settings_text = fs::read_to_string(settings_path)
        .map_err(|error| format!("Failed to read {}: {error}", settings_path.display()))?;
    let raw: AppSettingsFile = toml::from_str(&settings_text)
        .map_err(|error| format!("Failed to parse {}: {error}", settings_path.display()))?;
    let settings = AppSettings::from_file(raw, base_dir)?;
    Ok((settings, settings_path.to_path_buf()))
}

fn build_decks(raw: &AppSettingsFile, base_dir: &Path) -> Result<Vec<DeckConfig>, String> {
    if let Some(deck_files) = &raw.decks {
        if deck_files.is_empty() {
            return Err("`decks` cannot be empty.".to_string());
        }

        let decks = deck_files
            .iter()
            .enumerate()
            .map(|(index, deck)| {
                let image_dir = resolve_image_dir(base_dir, &deck.image_dir);
                let help_image = deck
                    .help_image
                    .as_ref()
                    .map(|path| resolve_image_dir(base_dir, path));
                let name = deck
                    .name
                    .clone()
                    .unwrap_or_else(|| default_deck_name(&image_dir, index));

                DeckConfig {
                    name,
                    image_dir,
                    help_image,
                }
            })
            .collect();

        return Ok(decks);
    }

    if let Some(image_dir) = &raw.image_dir {
        let resolved = resolve_image_dir(base_dir, image_dir);
        return Ok(vec![DeckConfig {
            name: default_deck_name(&resolved, 0),
            image_dir: resolved,
            help_image: None,
        }]);
    }

    Err("Settings must define either `image_dir` or `[[decks]]`.".to_string())
}

fn resolve_image_dir(base_dir: &Path, image_dir: &Path) -> PathBuf {
    if image_dir.is_absolute() {
        image_dir.to_path_buf()
    } else {
        base_dir.join(image_dir)
    }
}

fn default_deck_name(image_dir: &Path, index: usize) -> String {
    image_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Deck {}", index + 1))
}

fn relative_to(base_dir: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(base_dir)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn parse_key(value: &str) -> Result<Key, String> {
    let normalized = value.trim().to_ascii_lowercase();
    bindable_keys()
        .iter()
        .copied()
        .find(|key| key.label().eq_ignore_ascii_case(&normalized))
        .ok_or_else(|| format!("Unsupported key binding `{value}`"))
}

fn parse_color(value: &str) -> Result<egui::Color32, String> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return Err(format!("Unsupported color `{value}`. Use #RRGGBB."));
    }

    let red = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| format!("Unsupported color `{value}`. Use #RRGGBB."))?;
    let green = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| format!("Unsupported color `{value}`. Use #RRGGBB."))?;
    let blue = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| format!("Unsupported color `{value}`. Use #RRGGBB."))?;

    Ok(egui::Color32::from_rgb(red, green, blue))
}

fn color_to_hex(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "gma_runner_{name}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn duplicate_keybindings_are_rejected() {
        let bindings = KeyBindings {
            next_card: Key::Space,
            toggle_help: Key::H,
            toggle_deck_menu: Key::M,
            toggle_full_screen: Key::F,
            next_deck: Key::Tab,
            toggle_debug: Key::D,
            open_settings: Key::Space,
            quit: Key::Q,
        };

        let error = bindings
            .validate_unique()
            .expect_err("duplicate bindings should fail");
        assert!(error.contains("Duplicate key binding"));
        assert!(error.contains("Space"));
    }

    #[test]
    fn app_settings_from_file_resolves_relative_paths_and_clamps_active_deck() {
        let base_dir = Path::new("/tmp/gma-runner-tests");
        let raw = AppSettingsFile {
            image_dir: None,
            decks: Some(vec![
                DeckConfigFile {
                    name: Some("Deck A".to_string()),
                    image_dir: PathBuf::from("decks/a"),
                    help_image: Some(PathBuf::from("help/a.png")),
                },
                DeckConfigFile {
                    name: None,
                    image_dir: PathBuf::from("decks/b"),
                    help_image: None,
                },
            ]),
            active_deck: Some(99),
            card_max_width: 420.0,
            card_max_height: 640.0,
            help_max_width: 900.0,
            help_max_height: 1100.0,
            card_corner_radius: 16.0,
            preload_cards: true,
            keybindings: Some(KeyBindingsFile::default()),
            ui_style: Some(UiStyleFile::default()),
        };

        let settings = AppSettings::from_file(raw, base_dir).expect("settings should parse");

        assert_eq!(settings.active_deck, 1);
        assert_eq!(settings.decks[0].image_dir, base_dir.join("decks/a"));
        assert_eq!(
            settings.decks[0].help_image,
            Some(base_dir.join("help/a.png"))
        );
        assert_eq!(settings.decks[1].name, "b");
    }

    #[test]
    fn app_settings_to_file_serializes_relative_paths() {
        let base_dir = Path::new("/tmp/gma-runner-tests");
        let settings = AppSettings {
            decks: vec![DeckConfig {
                name: "Deck A".to_string(),
                image_dir: base_dir.join("decks/a"),
                help_image: Some(base_dir.join("help/a.png")),
            }],
            active_deck: 0,
            card_max_width: 420.0,
            card_max_height: 640.0,
            help_max_width: 900.0,
            help_max_height: 1100.0,
            card_corner_radius: 16.0,
            preload_cards: true,
            keybindings: KeyBindings::from_file(KeyBindingsFile::default())
                .expect("default keybindings should parse"),
            ui_style: UiStyle::from_file(UiStyleFile::default())
                .expect("default ui style should parse"),
        };

        let file = settings.to_file(base_dir);
        let decks = file.decks.expect("decks should be serialized");

        assert_eq!(decks[0].image_dir, PathBuf::from("decks/a"));
        assert_eq!(decks[0].help_image, Some(PathBuf::from("help/a.png")));
    }

    #[test]
    fn invalid_keybinding_is_rejected() {
        let error = KeyBindings::from_file(KeyBindingsFile {
            next_card: "F1".to_string(),
            ..KeyBindingsFile::default()
        })
        .expect_err("unsupported key should fail");

        assert!(error.contains("Unsupported key binding"));
    }

    #[test]
    fn missing_toggle_full_screen_defaults_to_f() {
        let raw: KeyBindingsFile = toml::from_str(
            r#"
next_card = "Space"
toggle_help = "H"
toggle_deck_menu = "M"
next_deck = "Tab"
toggle_debug = "D"
open_settings = "S"
quit = "Q"
"#,
        )
        .expect("legacy keybindings should deserialize");

        let bindings = KeyBindings::from_file(raw).expect("legacy keybindings should parse");

        assert_eq!(bindings.toggle_full_screen, Key::F);
    }

    #[test]
    fn invalid_color_is_rejected() {
        let error = UiStyle::from_file(UiStyleFile {
            toolbar_fill: "#12345".to_string(),
            ..UiStyleFile::default()
        })
        .expect_err("unsupported color should fail");

        assert!(error.contains("Use #RRGGBB"));
    }

    #[test]
    fn load_settings_from_roots_creates_settings_from_default() {
        let root = temp_test_dir("settings_fallback");
        let default_settings_path = root.join(DEFAULT_SETTINGS_FILE_NAME);
        fs::write(
            &default_settings_path,
            r#"
card_max_width = 420.0
card_max_height = 640.0
help_max_width = 900.0
help_max_height = 1100.0
card_corner_radius = 16.0
preload_cards = true

[[decks]]
name = "Demo Deck"
image_dir = "demo-assets/decks/demo-deck"
help_image = "demo-assets/help/demo-deck-help.png"
"#,
        )
        .expect("default settings should be written");

        let (settings, settings_path) =
            load_settings_from_roots([Some(root.clone())]).expect("settings should load");

        assert_eq!(settings_path, root.join(SETTINGS_FILE_NAME));
        assert!(settings_path.is_file());
        assert_eq!(settings.current_deck().name, "Demo Deck");
        assert_eq!(
            settings.current_deck().image_dir,
            root.join("demo-assets/decks/demo-deck")
        );

        let copied_settings = fs::read_to_string(&settings_path).expect("settings should be copied");
        let default_settings =
            fs::read_to_string(&default_settings_path).expect("default settings should be readable");
        assert_eq!(copied_settings, default_settings);
    }
}
