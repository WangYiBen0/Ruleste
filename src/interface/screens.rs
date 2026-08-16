//! User interface screens and state machine.
//!
//! This module implements the game's UI flow:
//! - Title screen (with mountain backdrop animation)
//! - Main menu (Play, Options, Credits, Exit)
//! - Save slot selection
//! - Pause menu (Resume, Restart, Map, Options, Exit)
//! - Chapter complete screen
//! - Dialog/cutscene system

use crate::data::font::SpriteFont;

/// Represents the current UI state of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiState {
    /// Initial loading state (assets being loaded)
    Loading,
    /// Title screen with mountain animation
    Title,
    /// Main menu navigation
    MainMenu,
    /// Save slot selection screen
    SaveSelect,
    /// Options menu
    Options,
    /// Credits screen
    Credits,
    /// In-game pause menu
    Pause,
    /// Chapter complete summary
    ChapterComplete,
    /// Dialog/cutscene overlay
    Dialog,
    /// Normal gameplay (no UI overlay)
    InGame,
}

/// Transitions between UI states based on user input.
#[derive(Debug)]
pub struct UiStateMachine {
    pub state: UiState,
    pub selected_index: usize,
    /// Timer for transitions and animations
    pub timer: f32,
}

impl UiStateMachine {
    pub fn new() -> Self {
        Self {
            state: UiState::Loading,
            selected_index: 0,
            timer: 0.0,
        }
    }

    /// Update the state machine, handling transitions based on input.
    pub fn update(&mut self, dt: f32, input: &crate::engine::input::Input) {
        use ruleste_plugin_api::types::input as act;

        self.timer += dt;

        match self.state {
            UiState::Loading => {
                // Auto-transition after assets loaded (handled by main loop)
            }
            UiState::Title => {
                if input.pressed(act::CONFIRM) || input.pressed(act::START) {
                    self.transition_to(UiState::MainMenu);
                }
            }
            UiState::MainMenu => {
                let menu_items = 4; // Play, Options, Credits, Exit

                if input.pressed(act::MOVE_UP) {
                    self.selected_index = self.selected_index.saturating_sub(1);
                }
                if input.pressed(act::MOVE_DOWN) {
                    self.selected_index = (self.selected_index + 1).min(menu_items - 1);
                }
                if input.pressed(act::CONFIRM) || input.pressed(act::START) {
                    match self.selected_index {
                        0 => self.transition_to(UiState::SaveSelect),
                        1 => self.transition_to(UiState::Options),
                        2 => self.transition_to(UiState::Credits),
                        3 => {
                            // Exit - signal quit to main loop
                            self.state = UiState::Title; // Placeholder
                        }
                        _ => {}
                    }
                }
            }
            UiState::SaveSelect => {
                let slots = 3; // 3 save slots

                if input.pressed(act::MOVE_LEFT) {
                    self.selected_index = self.selected_index.saturating_sub(1);
                }
                if input.pressed(act::MOVE_RIGHT) {
                    self.selected_index = (self.selected_index + 1).min(slots - 1);
                }
                if input.pressed(act::CANCEL) {
                    self.transition_to(UiState::MainMenu);
                }
                if input.pressed(act::CONFIRM) || input.pressed(act::START) {
                    // Start game with selected slot
                    self.transition_to(UiState::InGame);
                }
            }
            UiState::Pause => {
                let menu_items = 4; // Resume, Restart, Options, Exit

                if input.pressed(act::MOVE_UP) {
                    self.selected_index = self.selected_index.saturating_sub(1);
                }
                if input.pressed(act::MOVE_DOWN) {
                    self.selected_index = (self.selected_index + 1).min(menu_items - 1);
                }
                if input.pressed(act::CONFIRM) {
                    match self.selected_index {
                        0 => self.transition_to(UiState::InGame), // Resume
                        1 => {
                            // Restart - trigger level reload
                            self.transition_to(UiState::InGame);
                        }
                        2 => self.transition_to(UiState::Options),
                        3 => self.transition_to(UiState::MainMenu), // Exit to menu
                        _ => {}
                    }
                }
                if input.pressed(act::CANCEL) || input.pressed(act::PAUSE) {
                    self.transition_to(UiState::InGame);
                }
            }
            UiState::Options | UiState::Credits => {
                if input.pressed(act::CANCEL) || input.pressed(act::PAUSE) {
                    self.transition_to(UiState::MainMenu);
                }
            }
            UiState::ChapterComplete => {
                if input.pressed(act::CONFIRM) || input.pressed(act::START) {
                    self.transition_to(UiState::MainMenu);
                }
            }
            UiState::Dialog => {
                if input.pressed(act::CONFIRM) || input.pressed(act::START) {
                    // Advance dialog or close if complete
                    self.transition_to(UiState::InGame);
                }
            }
            UiState::InGame => {
                if input.pressed(act::PAUSE) {
                    self.transition_to(UiState::Pause);
                }
            }
        }
    }

    /// Transition to a new UI state, resetting relevant state.
    fn transition_to(&mut self, new_state: UiState) {
        self.state = new_state;
        self.selected_index = 0;
        self.timer = 0.0;
    }
}

impl Default for UiStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Title screen renderer.
pub struct TitleScreen;

impl TitleScreen {
    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        timer: f32,
    ) {
        // Clear to black
        renderer.clear();

        // Title text with subtle pulse
        let pulse = (timer * 2.0).sin() * 0.1 + 1.0;
        let alpha = (255.0 * pulse) as u8;
        renderer.draw_text_centered(font, "CELESTE", 160.0, 70.0, (255, 255, 255, alpha));

        // Subtitle
        renderer.draw_text_centered(
            font,
            "Press Start or Confirm",
            160.0,
            140.0,
            (180, 180, 180, 255),
        );

        // Version/build info
        renderer.draw_text_right(font, "Ruleste v0.1.0", 318.0, 175.0, (100, 100, 100, 255));
    }
}

/// Main menu renderer.
pub struct MainMenu;

impl MainMenu {
    const ITEMS: &'static [&'static str] = &["CLIMB", "Options", "Credits", "Exit"];

    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        selected_index: usize,
    ) {
        renderer.clear();

        // Title
        renderer.draw_text_centered(font, "CELESTE", 160.0, 40.0, (255, 255, 255, 255));

        // Menu items
        let start_y = 80.0;
        let item_height = 20.0;

        for (i, item) in Self::ITEMS.iter().enumerate() {
            let y = start_y + i as f32 * item_height;
            let color = if i == selected_index {
                (255, 255, 255, 255) // White for selected
            } else {
                (150, 150, 150, 180) // Dimmed for unselected
            };

            // Draw selection indicator (triangle)
            if i == selected_index {
                let indicator_x = 100.0;
                renderer.draw_text(font, ">", indicator_x, y, color);
            }

            renderer.draw_text_centered(font, item, 160.0, y, color);
        }

        // Footer
        renderer.draw_text_centered(
            font,
            "Use Arrow Keys + Confirm/Cancel",
            160.0,
            165.0,
            (100, 100, 100, 255),
        );
    }
}

/// Save slot selection renderer.
pub struct SaveSelect;

impl SaveSelect {
    const SLOTS: &'static [&'static str] = &["Slot 1", "Slot 2", "Slot 3"];

    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        selected_index: usize,
    ) {
        renderer.clear();

        // Title
        renderer.draw_text_centered(font, "SELECT SAVE", 160.0, 40.0, (255, 255, 255, 255));

        // Save slots
        let start_x = 40.0;
        let slot_width = 90.0;
        let slot_y = 100.0;

        for (i, slot) in Self::SLOTS.iter().enumerate() {
            let x = start_x + i as f32 * slot_width;
            let color = if i == selected_index {
                (255, 255, 255, 255)
            } else {
                (150, 150, 150, 180)
            };

            // Draw slot box (simple border)
            renderer.draw_text(font, &format!("[{}]", slot), x, slot_y, color);

            // Placeholder for save data
            renderer.draw_text(font, "Empty", x + 5.0, slot_y + 15.0, (80, 80, 80, 255));
        }

        // Footer
        renderer.draw_text_centered(
            font,
            "← → : Select slot | Confirm: Start | Cancel: Back",
            160.0,
            165.0,
            (100, 100, 100, 255),
        );
    }
}

/// Pause menu renderer.
pub struct PauseMenu;

impl PauseMenu {
    const ITEMS: &'static [&'static str] = &["Resume", "Restart", "Options", "Exit to Menu"];

    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        selected_index: usize,
    ) {
        // Semi-transparent background
        renderer.clear();

        // Menu title
        renderer.draw_text_centered(font, "PAUSED", 160.0, 50.0, (255, 255, 255, 255));

        // Menu items
        let start_y = 80.0;
        let item_height = 20.0;

        for (i, item) in Self::ITEMS.iter().enumerate() {
            let y = start_y + i as f32 * item_height;
            let color = if i == selected_index {
                (255, 255, 255, 255)
            } else {
                (150, 150, 150, 180)
            };

            if i == selected_index {
                renderer.draw_text(font, ">", 100.0, y, color);
            }

            renderer.draw_text_centered(font, item, 160.0, y, color);
        }
    }
}

/// Chapter complete screen renderer.
pub struct ChapterComplete;

impl ChapterComplete {
    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        chapter_name: &str,
        strawberries: u32,
        total_strawberries: u32,
    ) {
        renderer.clear();

        // Title
        renderer.draw_text_centered(font, "CHAPTER COMPLETE", 160.0, 40.0, (255, 255, 255, 255));

        // Chapter name
        renderer.draw_text_centered(font, chapter_name, 160.0, 70.0, (180, 180, 180, 255));

        // Strawberries
        let berry_text = format!("Strawberries: {}/{}", strawberries, total_strawberries);
        renderer.draw_text_centered(font, &berry_text, 160.0, 100.0, (255, 100, 100, 255));

        // Prompt
        renderer.draw_text_centered(
            font,
            "Press Confirm to Continue",
            160.0,
            140.0,
            (150, 150, 150, 255),
        );
    }
}

/// Simple dialog renderer.
pub struct Dialog;

impl Dialog {
    pub fn render(
        renderer: &mut crate::interface::renderer::Renderer,
        font: &SpriteFont,
        text: &str,
        speaker: Option<&str>,
    ) {
        let box_x = 10.0;
        let box_y = 120.0;
        let box_w = 300.0;
        let box_h = 50.0;

        // Dialog box background (semi-transparent)
        // Note: For proper transparency, we'd need a texture with alpha

        // Speaker name (if any)
        if let Some(name) = speaker {
            renderer.draw_text(font, name, box_x + 5.0, box_y + 5.0, (255, 200, 100, 255));
        }

        // Dialog text (wrapped)
        let text_y = if speaker.is_some() {
            box_y + 20.0
        } else {
            box_y + 10.0
        };

        renderer.draw_text_wrapped(
            font,
            text,
            box_x + 5.0,
            text_y,
            box_w - 10.0,
            (255, 255, 255, 255),
        );

        // Continue indicator
        renderer.draw_text_right(
            font,
            "▼",
            box_x + box_w,
            box_y + box_h - 10.0,
            (200, 200, 200, 255),
        );
    }
}
