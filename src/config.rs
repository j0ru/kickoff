use clap::ArgMatches;

use crate::color::Color;
use crate::font::Font;

use font_loader::system_fonts::FontPropertyBuilder;
use font_loader::system_fonts;

pub struct Config<'a> {
    pub color_background: Color,
    pub color_text: Color,
    pub color_text_query: Color,
    pub color_text_selected: Color,
    pub color_prompt: Color,
    pub prompt: String,
    pub padding: u32,
    pub font: Font<'a>,
}

impl Config<'_> {
    pub fn from_args(args: ArgMatches) -> Self {

        let color_background = Color::from(
            args
                .value_of("color-background")
                .unwrap()
                .parse::<css_color::Rgba>()
                .unwrap(),
        );
        let color_text = Color::from(
            args
                .value_of("color-text")
                .unwrap()
                .parse::<css_color::Rgba>()
                .unwrap(),
        );
        let color_text_selected = Color::from(
            args
                .value_of("color-text-selected")
                .unwrap()
                .parse::<css_color::Rgba>()
                .unwrap(),
        );
        let color_text_query = Color::from(
            args
                .value_of("color-text-query")
                .unwrap()
                .parse::<css_color::Rgba>()
                .unwrap(),
        );
        let color_prompt = Color::from(
            args
                .value_of("color-prompt")
                .unwrap()
                .parse::<css_color::Rgba>()
                .unwrap(),
        );

        let prompt = args.value_of("prompt").unwrap();
        let padding: u32 = args.value_of("padding").unwrap().parse().unwrap();
        let font_size: f32 = args.value_of("font-size").unwrap().parse().unwrap();

        // Font parsing
        let font_name = args.value_of("font").unwrap_or("");
        let font_builder = FontPropertyBuilder::new().family(font_name).build();
        let (font_data, _) =  system_fonts::get(&font_builder).unwrap();
        let font = Font {
            font: rusttype::Font::try_from_vec(font_data).expect("Error constructing Font"),
            scale: rusttype::Scale::uniform(font_size),
        };

        Config {
            font: font,
            padding: padding,
            prompt: prompt.to_string(),
            color_background: color_background,
            color_text: color_text,
            color_text_query: color_text_query,
            color_text_selected: color_text_selected,
            color_prompt: color_prompt,
        }
    }
}