use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use eframe::egui;
use egui::{Key, TextureHandle, Vec2};
use rand::prelude::IndexedRandom;
use rand::rng;

use crate::image_loader::{
    DecodedCard, decode_image_from_path, find_image_paths, fit_size, load_texture_from_decoded,
    start_background_loader,
};
use crate::settings::{
    AppSettings, KeyBindings, KeyDisplayExt, SETTINGS_FILE_NAME, bindable_keys,
    default_app_settings, load_settings,
};

pub struct CardApp {
    settings: AppSettings,
    settings_path: PathBuf,
    image_paths: Vec<PathBuf>,
    loaded_cards: HashMap<PathBuf, TextureHandle>,
    background_receiver: Option<Receiver<Result<DecodedCard, String>>>,
    draft_keybindings: Option<KeyBindings>,
    show_debug: bool,
    hide_ui_chrome: bool,
    show_deck_menu: bool,
    show_help: bool,
    show_settings: bool,
    current_path: Option<PathBuf>,
    current_texture: Option<TextureHandle>,
    current_help_texture: Option<TextureHandle>,
    error_message: Option<String>,
    settings_message: Option<String>,
}

impl CardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (settings, settings_path, image_paths, error_message) = initialize_app();
        let mut app = Self {
            settings,
            settings_path,
            image_paths,
            loaded_cards: HashMap::new(),
            background_receiver: None,
            draft_keybindings: None,
            show_debug: false,
            hide_ui_chrome: false,
            show_deck_menu: false,
            show_help: false,
            show_settings: false,
            current_path: None,
            current_texture: None,
            current_help_texture: None,
            error_message,
            settings_message: None,
        };

        if app.error_message.is_none() && !app.image_paths.is_empty() {
            app.show_random_card();
            app.start_preload_if_enabled(cc.egui_ctx.clone());
        }

        app
    }

    fn card_max_size(&self) -> Vec2 {
        Vec2::new(self.settings.card_max_width, self.settings.card_max_height)
    }

    fn help_max_size(&self) -> Vec2 {
        Vec2::new(self.settings.help_max_width, self.settings.help_max_height)
    }

    fn start_preload_if_enabled(&mut self, ctx: egui::Context) {
        if !self.settings.preload_cards {
            return;
        }

        let pending_paths = self
            .image_paths
            .iter()
            .filter(|path| !self.loaded_cards.contains_key(path.as_path()))
            .cloned()
            .collect::<Vec<_>>();

        if !pending_paths.is_empty() {
            self.background_receiver = Some(start_background_loader(
                ctx,
                pending_paths,
                self.card_max_size(),
            ));
        }
    }

    fn load_current_deck(&mut self, ctx: &egui::Context) {
        self.background_receiver = None;
        self.loaded_cards.clear();
        self.show_deck_menu = false;
        self.show_help = false;
        self.show_settings = false;
        self.current_path = None;
        self.current_texture = None;
        self.current_help_texture = None;
        self.error_message = None;
        self.settings_message = None;
        self.draft_keybindings = None;
        self.image_paths = find_image_paths(&self.settings.current_deck().image_dir);

        if self.image_paths.is_empty() {
            self.error_message = Some(format!(
                "No images found in `{}`.",
                self.settings.current_deck().image_dir.display()
            ));
            return;
        }

        self.show_random_card();
        self.start_preload_if_enabled(ctx.clone());
    }

    fn show_random_card(&mut self) {
        if self.image_paths.is_empty() {
            self.error_message = Some(format!(
                "No images found in `{}`.",
                self.settings.current_deck().image_dir.display()
            ));
            return;
        }

        let mut rng = rng();
        let next_path = if self.image_paths.len() == 1 {
            self.image_paths.first().cloned()
        } else {
            self.image_paths
                .iter()
                .filter(|path| Some(path.as_path()) != self.current_path.as_deref())
                .collect::<Vec<_>>()
                .choose(&mut rng)
                .map(|path| (*path).clone())
        };

        match next_path {
            Some(path) => {
                self.current_texture = self.loaded_cards.get(&path).cloned();
                self.current_path = Some(path);
                self.error_message = None;
            }
            None => {
                self.error_message = Some("Unable to choose a card image.".to_string());
            }
        }
    }

    fn drain_background_loader(&mut self, ctx: &egui::Context) {
        let mut disconnected = false;

        if let Some(receiver) = &self.background_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(Ok(decoded_card)) => {
                        if self.loaded_cards.contains_key(&decoded_card.path) {
                            continue;
                        }

                        let path = decoded_card.path.clone();
                        let texture = load_texture_from_decoded(ctx, decoded_card);
                        self.loaded_cards.insert(path.clone(), texture.clone());

                        if self.current_path.as_ref() == Some(&path)
                            && self.current_texture.is_none()
                        {
                            self.current_texture = Some(texture);
                        }
                    }
                    Ok(Err(error)) => {
                        self.error_message = Some(error);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if disconnected {
            self.background_receiver = None;
        }
    }

    fn ensure_current_texture_loaded(&mut self, ctx: &egui::Context) {
        let Some(path) = self.current_path.clone() else {
            return;
        };

        self.current_texture = self.ensure_texture_loaded(ctx, &path, self.card_max_size());
    }

    fn ensure_help_texture_loaded(&mut self, ctx: &egui::Context) {
        let Some(path) = self.settings.current_deck().help_image.clone() else {
            self.current_help_texture = None;
            return;
        };

        self.current_help_texture = self.ensure_texture_loaded(ctx, &path, self.help_max_size());
    }

    fn ensure_texture_loaded(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        max_size: Vec2,
    ) -> Option<TextureHandle> {
        if let Some(texture) = self.loaded_cards.get(path).cloned() {
            self.error_message = None;
            return Some(texture);
        }

        match decode_image_from_path(path, max_size) {
            Ok(decoded_card) => {
                let texture = load_texture_from_decoded(ctx, decoded_card);
                self.loaded_cards
                    .insert(path.to_path_buf(), texture.clone());
                self.error_message = None;
                Some(texture)
            }
            Err(error) => {
                self.error_message = Some(error);
                None
            }
        }
    }

    fn select_deck(&mut self, index: usize, ctx: &egui::Context) {
        if index >= self.settings.decks.len() || index == self.settings.active_deck {
            self.show_deck_menu = false;
            return;
        }

        self.settings.active_deck = index;
        self.load_current_deck(ctx);
        self.persist_active_deck();
    }

    fn set_help_visible(&mut self, visible: bool) {
        self.show_help = visible;
        if !visible {
            self.current_help_texture = None;
        }
    }

    fn advance_to_next_card(&mut self) {
        self.set_help_visible(false);
        self.show_random_card();
    }

    fn toggle_ui_chrome(&mut self) {
        self.hide_ui_chrome = !self.hide_ui_chrome;
    }

    fn save_settings(&mut self) -> Result<(), String> {
        let Some(base_dir) = self.settings_path.parent() else {
            return Err(format!(
                "Cannot determine parent directory for {}",
                self.settings_path.display()
            ));
        };

        let settings_file = self.settings.to_file(base_dir);
        let settings_text = toml::to_string_pretty(&settings_file)
            .map_err(|error| format!("Failed to serialize settings: {error}"))?;
        fs::write(&self.settings_path, settings_text).map_err(|error| {
            format!("Failed to write {}: {error}", self.settings_path.display())
        })?;

        Ok(())
    }

    fn persist_active_deck(&mut self) {
        if let Err(error) = self.save_settings() {
            self.error_message = Some(error);
            self.settings_message = None;
        } else {
            self.settings_message = Some(format!("Saved to {}", self.settings_path.display()));
            self.error_message = None;
        }
    }

    fn open_settings(&mut self) {
        self.show_settings = true;
        self.draft_keybindings = Some(self.settings.keybindings.clone());
        self.settings_message = None;
    }

    fn close_settings(&mut self) {
        self.show_settings = false;
        self.draft_keybindings = None;
    }

    fn current_deck_loaded_count(&self) -> usize {
        self.image_paths
            .iter()
            .filter(|path| self.loaded_cards.contains_key(path.as_path()))
            .count()
    }

    fn current_help_loaded(&self) -> bool {
        self.settings
            .current_deck()
            .help_image
            .as_ref()
            .is_some_and(|path| self.loaded_cards.contains_key(path.as_path()))
    }

    fn handle_input(&mut self, ctx: &egui::Context, keybindings: &KeyBindings) -> bool {
        if ctx.input(|input| input.key_pressed(keybindings.quit)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return true;
        }

        if ctx.input(|input| input.key_pressed(keybindings.toggle_debug)) {
            self.show_debug = !self.show_debug;
        }
        if ctx.input(|input| input.key_pressed(keybindings.toggle_full_screen)) {
            self.toggle_ui_chrome();
        }
        if ctx.input(|input| input.key_pressed(keybindings.toggle_deck_menu)) {
            self.show_deck_menu = !self.show_deck_menu;
        }
        if ctx.input(|input| input.key_pressed(keybindings.toggle_help)) {
            self.set_help_visible(!self.show_help);
        }
        if ctx.input(|input| input.key_pressed(keybindings.next_deck)) {
            self.settings.advance_deck();
            self.load_current_deck(ctx);
            self.persist_active_deck();
        }
        if ctx.input(|input| input.key_pressed(keybindings.open_settings)) {
            if self.show_settings {
                self.close_settings();
            } else {
                self.open_settings();
            }
        }
        if ctx.input(|input| input.key_pressed(keybindings.next_card)) {
            self.advance_to_next_card();
        }

        false
    }

    fn sync_textures(&mut self, ctx: &egui::Context) {
        self.drain_background_loader(ctx);
        self.ensure_current_texture_loaded(ctx);
        if self.show_help {
            self.ensure_help_texture_loaded(ctx);
        }
    }

    fn display_texture(&self) -> Option<TextureHandle> {
        if self.show_help {
            self.current_help_texture.clone()
        } else {
            self.current_texture.clone()
        }
    }

    fn render_main_panel(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        keybindings: &KeyBindings,
    ) {
        let display_texture = self.display_texture();

        if self.hide_ui_chrome {
            self.render_card_only_ui(ctx, ui, keybindings, &display_texture);
        } else {
            self.render_full_ui(ctx, ui, keybindings, &display_texture);
        }
    }

    fn render_card_only_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        keybindings: &KeyBindings,
        display_texture: &Option<TextureHandle>,
    ) {
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| self.render_card_content(ui, display_texture, false),
        );
        self.render_restore_chrome_button(ctx, keybindings);
    }

    fn render_full_ui(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        keybindings: &KeyBindings,
        display_texture: &Option<TextureHandle>,
    ) {
        ui.vertical_centered(|ui| {
            self.render_header(ui, keybindings);

            if self.show_debug {
                self.render_debug_info(ui);
            }

            if let Some(error_message) = &self.error_message {
                ui.add_space(12.0);
                ui.colored_label(egui::Color32::RED, error_message);
                ui.label(format!(
                    "Update {} to fix the image path or card size.",
                    self.settings_path.display()
                ));
            }

            self.render_card_content(ui, display_texture, true);
            self.render_toolbar(ctx, ui, keybindings);

            ui.add_space(8.0);
            ui.small(format!(
                "Press {} to hide UI. Press {} to toggle debug details.",
                keybindings.toggle_full_screen.label(),
                keybindings.toggle_debug.label()
            ));
        });
    }

    fn render_header(&self, ui: &mut egui::Ui, keybindings: &KeyBindings) {
        ui.heading("Deck Shuffle Draw");
        ui.label(format!("Deck: {}", self.settings.current_deck().name));
        ui.label(format!(
            "Press {} for next card, {} for help, {} for deck menu, {} for full screen, {} for settings, {} for next deck, {} to quit.",
            keybindings.next_card.label(),
            keybindings.toggle_help.label(),
            keybindings.toggle_deck_menu.label(),
            keybindings.toggle_full_screen.label(),
            keybindings.open_settings.label(),
            keybindings.next_deck.label(),
            keybindings.quit.label(),
        ));
    }

    fn render_debug_info(&self, ui: &mut egui::Ui) {
        let deck_loaded_count = self.current_deck_loaded_count();
        ui.label("Debug: on");
        ui.label(format!(
            "Images: {}",
            self.settings.current_deck().image_dir.display()
        ));
        ui.label(format!(
            "Card size: {:.0} x {:.0}",
            self.settings.card_max_width, self.settings.card_max_height
        ));
        ui.label(format!(
            "Help size: {:.0} x {:.0}",
            self.settings.help_max_width, self.settings.help_max_height
        ));
        ui.label(format!(
            "Corner radius: {:.0}",
            self.settings.card_corner_radius
        ));
        ui.label(format!(
            "Preload cards: {}",
            if self.settings.preload_cards {
                "on"
            } else {
                "off"
            }
        ));
        ui.label(format!(
            "{}: {} of {}",
            if self.settings.preload_cards {
                "Cached cards"
            } else {
                "Cards loaded on demand"
            },
            deck_loaded_count,
            self.image_paths.len()
        ));
        ui.label(format!(
            "Help image: {}",
            self.settings
                .current_deck()
                .help_image
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        ui.label(format!(
            "Help loaded: {}",
            if self.current_help_loaded() {
                "yes"
            } else {
                "no"
            }
        ));

        if let Some(path) = &self.current_path {
            ui.label(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(""),
            );
        }
    }

    fn render_card_content(
        &mut self,
        ui: &mut egui::Ui,
        display_texture: &Option<TextureHandle>,
        add_top_spacing: bool,
    ) {
        if let Some(texture) = display_texture {
            if add_top_spacing {
                ui.add_space(12.0);
            }
            let max_size = if self.show_help {
                self.help_max_size()
            } else {
                self.card_max_size()
            };
            let available = ui.available_size().min(max_size);
            let fitted = fit_size(texture.size_vec2(), available);
            let response = ui.add(
                egui::Image::new(texture)
                    .fit_to_exact_size(fitted)
                    .sense(mouse_click_sense())
                    .corner_radius(self.settings.card_corner_radius.round() as u8),
            );
            if !self.show_help && response.clicked() {
                self.advance_to_next_card();
            }
        } else if !add_top_spacing {
            if let Some(error_message) = &self.error_message {
                ui.colored_label(egui::Color32::RED, error_message);
            }
        }
    }

    fn render_toolbar(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        keybindings: &KeyBindings,
    ) {
        ui.add_space(12.0);
        let toolbar_width = toolbar_preferred_width(8).min(ui.available_width());
        ui.allocate_ui_with_layout(
            egui::vec2(toolbar_width, 0.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                egui::Frame::new()
                    .fill(self.settings.ui_style.toolbar_fill)
                    .stroke(egui::Stroke::new(
                        TOOLBAR_STROKE_WIDTH,
                        self.settings.ui_style.toolbar_stroke,
                    ))
                    .corner_radius(TOOLBAR_CORNER_RADIUS)
                    .inner_margin(egui::Margin::symmetric(
                        TOOLBAR_INNER_MARGIN_X as i8,
                        TOOLBAR_INNER_MARGIN_Y as i8,
                    ))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing =
                            egui::vec2(TOOLBAR_ITEM_SPACING, TOOLBAR_ITEM_SPACING);
                        ui.horizontal_wrapped(|ui| {
                            if control_button(
                                ui,
                                format!("Next Card ({})", keybindings.next_card.label()),
                                false,
                                self.settings.ui_style.next_card,
                            )
                            .clicked()
                            {
                                self.advance_to_next_card();
                            }
                            if control_button(
                                ui,
                                format!("Help ({})", keybindings.toggle_help.label()),
                                self.show_help,
                                self.settings.ui_style.help,
                            )
                            .clicked()
                            {
                                self.set_help_visible(!self.show_help);
                            }
                            if control_button(
                                ui,
                                format!("Deck Menu ({})", keybindings.toggle_deck_menu.label()),
                                self.show_deck_menu,
                                self.settings.ui_style.deck_menu,
                            )
                            .clicked()
                            {
                                self.show_deck_menu = !self.show_deck_menu;
                            }
                            if control_button(
                                ui,
                                format!("Next Deck ({})", keybindings.next_deck.label()),
                                false,
                                self.settings.ui_style.next_deck,
                            )
                            .clicked()
                            {
                                self.settings.advance_deck();
                                self.load_current_deck(ctx);
                                self.persist_active_deck();
                            }
                            if control_button(
                                ui,
                                format!("Full Screen ({})", keybindings.toggle_full_screen.label()),
                                self.hide_ui_chrome,
                                FULL_SCREEN_BUTTON_TONE,
                            )
                            .clicked()
                            {
                                self.toggle_ui_chrome();
                            }
                            if control_button(
                                ui,
                                format!("Debug ({})", keybindings.toggle_debug.label()),
                                self.show_debug,
                                self.settings.ui_style.debug,
                            )
                            .clicked()
                            {
                                self.show_debug = !self.show_debug;
                            }
                            if control_button(
                                ui,
                                format!("Settings ({})", keybindings.open_settings.label()),
                                self.show_settings,
                                self.settings.ui_style.settings,
                            )
                            .clicked()
                            {
                                if self.show_settings {
                                    self.close_settings();
                                } else {
                                    self.open_settings();
                                }
                            }
                            if control_button(
                                ui,
                                format!("Quit ({})", keybindings.quit.label()),
                                false,
                                self.settings.ui_style.quit,
                            )
                            .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });
            },
        );
    }

    fn render_restore_chrome_button(&mut self, ctx: &egui::Context, keybindings: &KeyBindings) {
        egui::Area::new(egui::Id::new("restore_ui_chrome_button"))
            .anchor(egui::Align2::RIGHT_TOP, [-12.0, 12.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let label = format!("Show UI ({})", keybindings.toggle_full_screen.label());
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(label)
                            .small()
                            .strong()
                            .color(egui::Color32::from_white_alpha(210)),
                    )
                    .sense(mouse_click_sense())
                    .fill(egui::Color32::from_black_alpha(78))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_white_alpha(44)))
                    .corner_radius(12),
                );

                if response.clicked() {
                    self.toggle_ui_chrome();
                }
            });
    }

    fn render_deck_menu(&mut self, ctx: &egui::Context) {
        if !self.show_deck_menu {
            return;
        }

        let deck_names = self
            .settings
            .decks
            .iter()
            .map(|deck| deck.name.clone())
            .collect::<Vec<_>>();
        let active_deck = self.settings.active_deck;
        let mut selected_deck = None;
        let mut close_menu = false;

        egui::Window::new("Choose Deck")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Select a deck:");
                ui.add_space(8.0);

                for (index, name) in deck_names.iter().enumerate() {
                    let label = if index == active_deck {
                        format!("{name} (Current)")
                    } else {
                        name.clone()
                    };

                    if ui.button(label).clicked() {
                        selected_deck = Some(index);
                    }
                }

                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    close_menu = true;
                }
            });

        if let Some(index) = selected_deck {
            self.select_deck(index, ctx);
        } else if close_menu {
            self.show_deck_menu = false;
        }
    }

    fn render_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let mut close_settings = false;

        egui::Window::new("Settings")
            .anchor(egui::Align2::RIGHT_TOP, [-16.0, 16.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Key Bindings");
                ui.add_space(8.0);

                let draft = self
                    .draft_keybindings
                    .get_or_insert_with(|| self.settings.keybindings.clone());
                let snapshot = draft.clone();
                key_binding_row(
                    ui,
                    "Next Card",
                    &mut draft.next_card,
                    &[
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Help",
                    &mut draft.toggle_help,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Deck Menu",
                    &mut draft.toggle_deck_menu,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Full Screen",
                    &mut draft.toggle_full_screen,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Next Deck",
                    &mut draft.next_deck,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Debug",
                    &mut draft.toggle_debug,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.open_settings,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Settings",
                    &mut draft.open_settings,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.quit,
                    ],
                );
                key_binding_row(
                    ui,
                    "Quit",
                    &mut draft.quit,
                    &[
                        snapshot.next_card,
                        snapshot.toggle_help,
                        snapshot.toggle_deck_menu,
                        snapshot.toggle_full_screen,
                        snapshot.next_deck,
                        snapshot.toggle_debug,
                        snapshot.open_settings,
                    ],
                );

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked()
                        && let Some(draft) = self.draft_keybindings.clone()
                    {
                        match draft.validate_unique() {
                            Ok(()) => {
                                self.settings.keybindings = draft;
                                match self.save_settings() {
                                    Ok(()) => {
                                        self.settings_message = Some(format!(
                                            "Saved to {}",
                                            self.settings_path.display()
                                        ));
                                        self.error_message = None;
                                    }
                                    Err(error) => {
                                        self.error_message = Some(error);
                                        self.settings_message = None;
                                    }
                                }
                            }
                            Err(error) => {
                                self.error_message = Some(error);
                                self.settings_message = None;
                            }
                        }
                    }

                    if ui.button("Close").clicked() {
                        close_settings = true;
                    }
                });

                if let Some(message) = &self.settings_message {
                    ui.add_space(8.0);
                    ui.label(message);
                }
            });

        if close_settings {
            self.close_settings();
        }
    }
}

const CONTROL_BUTTON_MIN_WIDTH: f32 = 136.0;
const CONTROL_BUTTON_MIN_HEIGHT: f32 = 34.0;
const TOOLBAR_ITEM_SPACING: f32 = 8.0;
const TOOLBAR_INNER_MARGIN_X: f32 = 12.0;
const TOOLBAR_INNER_MARGIN_Y: f32 = 10.0;
const TOOLBAR_CORNER_RADIUS: u8 = 12;
const TOOLBAR_STROKE_WIDTH: f32 = 1.0;
const FULL_SCREEN_BUTTON_TONE: egui::Color32 = egui::Color32::from_rgb(62, 94, 112);

fn mouse_click_sense() -> egui::Sense {
    egui::Sense::CLICK
}

fn control_button(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    active: bool,
    tone: egui::Color32,
) -> egui::Response {
    let min_size = egui::vec2(CONTROL_BUTTON_MIN_WIDTH, CONTROL_BUTTON_MIN_HEIGHT);

    let label = label.into();
    let fill = if active {
        tone
    } else {
        tone.gamma_multiply(0.45)
    };
    let stroke = if active {
        egui::Stroke::new(1.5, tone.gamma_multiply(0.75))
    } else {
        egui::Stroke::new(1.0, tone.gamma_multiply(0.65))
    };

    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .strong()
                .color(egui::Color32::WHITE),
        )
        .sense(mouse_click_sense())
        .min_size(min_size)
        .fill(fill)
        .stroke(stroke),
    )
}

fn toolbar_preferred_width(button_count: usize) -> f32 {
    let button_count = button_count as f32;
    let gaps = (button_count - 1.0).max(0.0);

    button_count * CONTROL_BUTTON_MIN_WIDTH
        + gaps * TOOLBAR_ITEM_SPACING
        + 2.0 * TOOLBAR_INNER_MARGIN_X
        + 2.0 * TOOLBAR_STROKE_WIDTH
}

fn available_bindable_keys(unavailable_keys: &[Key]) -> Vec<Key> {
    bindable_keys()
        .iter()
        .copied()
        .filter(|key| !unavailable_keys.contains(key))
        .collect()
}

fn key_binding_row(ui: &mut egui::Ui, label: &str, binding: &mut Key, unavailable_keys: &[Key]) {
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .selected_text(binding.label())
            .show_ui(ui, |ui| {
                for key in available_bindable_keys(unavailable_keys) {
                    ui.selectable_value(binding, key, key.label());
                }
            });
    });
}

fn initialize_app() -> (AppSettings, PathBuf, Vec<PathBuf>, Option<String>) {
    let (settings, settings_path) = match load_settings() {
        Ok(result) => result,
        Err(error) => {
            return (
                default_app_settings(),
                PathBuf::from(SETTINGS_FILE_NAME),
                Vec::new(),
                Some(error),
            );
        }
    };

    let image_paths = find_image_paths(&settings.current_deck().image_dir);
    if image_paths.is_empty() {
        let error_message = format!(
            "No images found in `{}`.",
            settings.current_deck().image_dir.display()
        );
        return (settings, settings_path, Vec::new(), Some(error_message));
    }

    (settings, settings_path, image_paths, None)
}

impl eframe::App for CardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let keybindings = self.settings.keybindings.clone();
        if self.handle_input(ctx, &keybindings) {
            return;
        }

        self.sync_textures(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_main_panel(ctx, ui, &keybindings);
        });

        self.render_deck_menu(ctx);
        self.render_settings_window(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_bindable_keys_excludes_unavailable_keys() {
        let available = available_bindable_keys(&[Key::F, Key::Q, Key::Space]);

        assert!(!available.contains(&Key::F));
        assert!(!available.contains(&Key::Q));
        assert!(!available.contains(&Key::Space));
        assert!(available.contains(&Key::H));
        assert!(available.contains(&Key::Tab));
    }
}
