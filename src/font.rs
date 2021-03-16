use crate::color::Color;

use std::collections::HashMap;

use font_loader::system_fonts;
use font_loader::system_fonts::FontPropertyBuilder;

use fontdue::{layout::GlyphRasterConfig, FontSettings, Metrics};

use image::{Pixel, RgbaImage};

pub struct Font {
    font: fontdue::Font,
    scale: f32,
    glyph_cache: HashMap<GlyphRasterConfig, (Metrics, Vec<u8>)>,
}

impl Font {
    pub fn new(name: &str, size: f32) -> Font {
        let font_builder = FontPropertyBuilder::new().family(name).build();
        let (font_data, _) = system_fonts::get(&font_builder).unwrap();
        Font {
            font: fontdue::Font::from_bytes(font_data, FontSettings::default())
                .expect("Failed to parse Font"),
            scale: size,
            glyph_cache: HashMap::new(),
        }
    }

    pub async fn new_async(name: &str, size: f32) -> Font {
        Font::new(name, size)
    }

    fn render_glyph(&mut self, conf: GlyphRasterConfig) -> (Metrics, Vec<u8>) {
        if let Some(bitmap) = self.glyph_cache.get(&conf) {
            bitmap.clone()
        } else {
            self.glyph_cache
                .insert(conf, self.font.rasterize_config(conf));
            self.glyph_cache.get(&conf).unwrap().clone()
        }
    }

    pub fn render(
        &mut self,
        text: &str,
        color: &Color,
        image: &mut RgbaImage,
        x_offset: u32,
        y_offset: u32,
    ) -> (u32, u32) {
        let mut width = 0.;
        for letter in text.chars() {
            let (meta, bitmap) = self.render_glyph(GlyphRasterConfig {
                c: letter,
                px: self.scale,
                font_index: 0,
            });
            for (i, alpha) in bitmap.iter().enumerate() {
                if alpha != &0 {
                    let x = ((i % meta.width) as f32 + width + x_offset as f32 + meta.xmin as f32)
                        as u32;
                    let y = ((i as f32 / meta.width as f32) + y_offset as f32 + self.scale
                        - meta.height as f32
                        - meta.ymin as f32) as u32;
                    if x < image.width() && y < image.height() {
                        if alpha == &255 {
                            image.put_pixel(x, y, image::Rgba([color.0, color.1, color.2, 255]));
                        } else {
                            image
                                .get_pixel_mut(x, y)
                                .blend(&image::Rgba([color.0, color.1, color.2, *alpha]));
                        }
                    }
                }
            }
            width += meta.advance_width;
        }

        (width as u32, self.scale as u32)
    }
}
