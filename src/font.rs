use crate::color::Color;

use font_loader::system_fonts;
use font_loader::system_fonts::FontPropertyBuilder;

use image::{Pixel, RgbaImage};

pub struct Font<'a> {
    pub font: rusttype::Font<'a>,
    pub scale: rusttype::Scale,
}

impl Font<'_> {
    pub fn new(name: &str, size: f32) -> Font {
        let font_builder = FontPropertyBuilder::new().family(name).build();
        let (font_data, _) = system_fonts::get(&font_builder).unwrap();
        Font {
            font: rusttype::Font::try_from_vec(font_data).expect("Error constructing Font"),
            scale: rusttype::Scale::uniform(size),
        }
    }

    pub async fn new_async(name: &str, size: f32) -> Font<'_> {
        Font::new(name, size)
    }

    pub fn render(
        &self,
        text: &str,
        color: &Color,
        image: &mut RgbaImage,
        x_offset: u32,
        y_offset: u32,
    ) -> (u32, u32) {
        let v_metrics = self.font.v_metrics(self.scale);

        let glyphs: Vec<_> = self
            .font
            .layout(text, self.scale, rusttype::point(0.0, v_metrics.ascent))
            .collect();
        let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
        let glyphs_width = glyphs
            .iter()
            .rev()
            .map(|g| g.position().x as f32 + g.unpositioned().h_metrics().advance_width)
            .next()
            .unwrap_or(0.0)
            .ceil() as u32;

        for glyph in glyphs {
            if let Some(bounding_box) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let x = x + bounding_box.min.x as u32 + x_offset;
                    let y = y + bounding_box.min.y as u32 + y_offset;
                    if x < image.width() && y < image.height() {
                        image.get_pixel_mut(x, y).blend(&image::Rgba([
                            color.0,
                            color.1,
                            color.2,
                            (v * color.3 as f32) as u8,
                        ]));
                    }
                });
            }
        }
        (glyphs_width, glyphs_height)
    }
}
