use crate::color::Color;
use confy;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub color_background: Color,
    pub color_text: Color,
    pub color_text_query: Color,
    pub color_text_selected: Color,
    pub color_prompt: Color,
    pub prompt: String,
    pub padding: u32,
    pub font: String,
    pub font_size: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            color_background: Color(40, 44, 52, 170),
            color_prompt: Color(171, 178, 191, 255),
            color_text: Color(255, 255, 255, 255),
            color_text_query: Color(229, 192, 123, 255),
            color_text_selected: Color(97, 175, 239, 255),
            prompt: "".to_owned(),
            padding: 100,
            font: "".to_owned(),
            font_size: 32.,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, confy::ConfyError> {
        confy::load("kickoff")
    }
}
