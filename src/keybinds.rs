use crate::gui::Action;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use smithay_client_toolkit::seat::keyboard::ModifiersState;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use x11_keysymdef::lookup_by_name;

use crate::config::KeybindingsConfig;

pub struct Keybindings {
    inner: HashMap<KeyCombo, Action>,
}

impl From<KeybindingsConfig> for Keybindings {
    fn from(config: KeybindingsConfig) -> Self {
        let mut res = Keybindings {
            inner: HashMap::new(),
        };

        res.add_key_combos(Action::Complete, &config.complete);
        res.add_key_combos(Action::Execute, &config.execute);
        res.add_key_combos(Action::Exit, &config.exit);
        res.add_key_combos(Action::Delete, &config.delete);
        res.add_key_combos(Action::DeleteWord, &config.delete_word);
        res.add_key_combos(Action::NavUp, &config.nav_up);
        res.add_key_combos(Action::NavDown, &config.nav_down);
        res.add_key_combos(Action::Paste, &config.paste);

        res
    }
}

#[derive(Clone, Default, Debug)]
pub struct Modifiers(ModifiersState);

impl From<ModifiersState> for Modifiers {
    fn from(modifiers: ModifiersState) -> Self {
        Modifiers(modifiers)
    }
}

impl Hash for Modifiers {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.alt.hash(state);
        self.0.shift.hash(state);
        self.0.ctrl.hash(state);
        self.0.logo.hash(state);
    }
}

impl PartialEq for Modifiers {
    fn eq(&self, other: &Modifiers) -> bool {
        self.0.ctrl == other.0.ctrl
            && self.0.alt == other.0.alt
            && self.0.shift == other.0.shift
            && self.0.logo == other.0.logo
    }
}
impl Eq for Modifiers {}

#[derive(Eq, PartialEq, Hash, Clone, fmt::Debug)]
pub struct KeyCombo {
    modifiers: Modifiers,
    key: u32,
}

impl Keybindings {
    pub fn get(&self, modifiers: &ModifiersState, keysym: u32) -> Option<&Action> {
        self.inner.get(&KeyCombo {
            modifiers: Modifiers(*modifiers),
            key: keysym,
        })
    }

    fn add_key_combos(&mut self, action: Action, key_combos: &[KeyCombo]) {
        key_combos.iter().for_each(|entry| {
            self.inner.insert(entry.to_owned(), action);
        });
    }
}

impl KeyCombo {
    pub fn new(modifiers: Modifiers, key: u32) -> Self {
        KeyCombo { modifiers, key }
    }
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(KeyComboVisitor)
    }
}

struct KeyComboVisitor;
impl<'de> Visitor<'de> for KeyComboVisitor {
    type Value = KeyCombo;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("assignments of key combinations")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let mut modifiers = ModifiersState::default();
        let mut key: Option<u32> = None;
        value.split('+').for_each(|s| match s {
            "ctrl" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            "logo" => modifiers.logo = true,
            s => {
                if let Some(value) = lookup_by_name(s) {
                    key = Some(value.keysym);
                }
            }
        });
        if let Some(key) = key {
            Ok(KeyCombo {
                modifiers: Modifiers(modifiers),
                key,
            })
        } else {
            Err(de::Error::custom("No key given or unable to parse"))
        }
    }
}
