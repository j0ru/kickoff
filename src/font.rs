use crate::color::Color;
use fontdue::layout::{CoordinateSystem, GlyphRasterConfig, Layout, LayoutSettings, TextStyle};
use fontdue::Metrics;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};

use fontconfig::Fontconfig;

use image::{Pixel, RgbaImage};

pub struct Font {
    fonts: Vec<fontdue::Font>,
    layout: RefCell<Layout>,
    scale: f32,
    glyph_cache: RefCell<HashMap<GlyphRasterConfig, (Metrics, Vec<u8>)>>,
}

impl Font {
    pub async fn new(font_names: Vec<String>, size: f32) -> io::Result<Font> {
        let fc = Fontconfig::new().expect("Couldn't load fontconfig");
        let font_names = if font_names.len() == 0 {
            vec![String::new()]
        } else {
            font_names
        };
        let font_paths: Vec<PathBuf> = font_names
            .iter()
            .map(|name| fc.find(name, None).unwrap().path)
            .collect();
        let mut font_data = Vec::new();

        for font_path in font_paths {
            let mut font_buffer = Vec::new();
            File::open(font_path.to_str().unwrap())
                .await?
                .read_to_end(&mut font_buffer)
                .await?;
            font_data.push(
                fontdue::Font::from_bytes(font_buffer, fontdue::FontSettings::default()).unwrap(),
            );
        }

        Ok(Font {
            fonts: font_data,
            layout: RefCell::new(Layout::new(CoordinateSystem::PositiveYDown)),
            scale: size,
            glyph_cache: RefCell::new(HashMap::new()),
        })
    }

    fn render_glyph(&self, conf: GlyphRasterConfig) -> (Metrics, Vec<u8>) {
        let mut glyph_cache = self.glyph_cache.borrow_mut();
        if let Some(bitmap) = glyph_cache.get(&conf) {
            bitmap.clone()
        } else {
            let font: Vec<&fontdue::Font> = self
                .fonts
                .iter()
                .filter(|f| (*f).file_hash() == conf.font_hash)
                .collect();
            glyph_cache.insert(conf, font.first().unwrap().rasterize_config(conf));
            glyph_cache.get(&conf).unwrap().clone()
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
        let mut width = 0;
        let mut layout = self.layout.borrow_mut();
        layout.reset(&LayoutSettings::default());
        for c in text.chars() {
            let mut font_index = 0;
            for (i, font) in self.fonts.iter().enumerate() {
                if font.lookup_glyph_index(c) != 0 {
                    font_index = i;
                    break;
                }
            }
            layout.append(
                &self.fonts,
                &TextStyle::new(&c.to_string(), self.scale, font_index),
            );
        }

        for glyph in layout.glyphs() {
            let (_, bitmap) = self.render_glyph(glyph.key);
            for (i, alpha) in bitmap.iter().enumerate() {
                if alpha != &0 {
                    let x = glyph.x + x_offset as f32 + (i % glyph.width) as f32;
                    let y = glyph.y + y_offset as f32 + (i / glyph.width) as f32;

                    image
                        .get_pixel_mut(x as u32, y as u32)
                        .blend(&image::Rgba([color.0, color.1, color.2, *alpha]));
                }
            }
        }
        if let Some(glyph) = layout.glyphs().last() {
            width = glyph.x as usize + glyph.width;
        }

        (width as u32, layout.height() as u32)
    }
}
