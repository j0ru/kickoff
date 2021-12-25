use crate::gui::Action;
use smithay_client_toolkit::seat::keyboard::{keysyms, ModifiersState};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct Keybindings {
    inner: HashMap<KeyCombo, Action>,
}

struct Modifiers(ModifiersState);

impl Hash for Modifiers {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.alt.hash(state);
        self.0.shift.hash(state);
        self.0.ctrl.hash(state);
        self.0.caps_lock.hash(state);
        self.0.logo.hash(state);
        self.0.num_lock.hash(state);
    }
}
impl PartialEq for Modifiers {
    fn eq(&self, other: &Modifiers) -> bool {
        self.0.ctrl == other.0.ctrl
            && self.0.alt == other.0.alt
            && self.0.shift == other.0.shift
            && self.0.caps_lock == other.0.caps_lock
            && self.0.logo == other.0.logo
            && self.0.num_lock == other.0.num_lock
    }
}
impl Eq for Modifiers {}

#[derive(Eq, PartialEq, Hash)]
struct KeyCombo {
    modifiers: Modifiers,
    key: u32,
}

impl Keybindings {
    pub fn get(&self, modifiers: &ModifiersState, keysym: u32) -> Option<&Action> {
        self.inner.get(&KeyCombo {
            modifiers: Modifiers(modifiers.clone()),
            key: keysym,
        })
    }
}

impl Default for Keybindings {
    fn default() -> Self {
        Keybindings {
            inner: HashMap::from([
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Delete,
                    },
                    Action::Delete,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_BackSpace,
                    },
                    Action::Delete,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_KP_Delete,
                    },
                    Action::Delete,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Tab,
                    },
                    Action::Complete,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Return,
                    },
                    Action::Execute,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_KP_Enter,
                    },
                    Action::Execute,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Up,
                    },
                    Action::NavUp,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_KP_Up,
                    },
                    Action::NavUp,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Down,
                    },
                    Action::NavDown,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_KP_Down,
                    },
                    Action::NavDown,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState::default()),
                        key: keysyms::XKB_KEY_Escape,
                    },
                    Action::Exit,
                ),
                (
                    KeyCombo {
                        modifiers: Modifiers(ModifiersState {
                            ctrl: true,
                            ..ModifiersState::default()
                        }),
                        key: keysyms::XKB_KEY_v,
                    },
                    Action::Paste,
                ),
            ]),
        }
    }
}
