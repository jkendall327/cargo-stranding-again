use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GameScreen {
    #[default]
    Playing,
    PauseMenu,
    InventoryMenu,
    OptionsMenu,
}

impl GameScreen {
    pub fn allows_simulation(self) -> bool {
        matches!(self, Self::Playing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauseMenuEntry {
    Resume,
    Options,
}

impl PauseMenuEntry {
    pub const ALL: [Self; 2] = [Self::Resume, Self::Options];

    pub fn label(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Options => "Options",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MenuSelection {
    selected_index: usize,
}

impl MenuSelection {
    pub fn selected_index(self) -> usize {
        self.selected_index
    }

    pub fn select_next(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + 1) % len;
        }
    }

    pub fn select_previous(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + len - 1) % len;
        }
    }

    pub fn clamp_to_len(&mut self, len: usize) {
        if len == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct PauseMenuState {
    selection: MenuSelection,
}

impl PauseMenuState {
    pub fn selected(self) -> PauseMenuEntry {
        PauseMenuEntry::ALL[self.selection.selected_index()]
    }

    pub fn select_next(&mut self) {
        self.selection.select_next(PauseMenuEntry::ALL.len());
    }

    pub fn select_previous(&mut self) {
        self.selection.select_previous(PauseMenuEntry::ALL.len());
    }
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct InventoryMenuState {
    selection: MenuSelection,
}

impl InventoryMenuState {
    pub fn selected_index(self) -> usize {
        self.selection.selected_index()
    }

    pub fn select_next(&mut self, item_count: usize) {
        self.selection.select_next(item_count);
    }

    pub fn select_previous(&mut self, item_count: usize) {
        self.selection.select_previous(item_count);
    }

    pub fn clamp_to_item_count(&mut self, item_count: usize) {
        self.selection.clamp_to_len(item_count);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryAction {
    DropSelected,
}

#[derive(Resource, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InventoryIntent {
    pub action: Option<InventoryAction>,
}
