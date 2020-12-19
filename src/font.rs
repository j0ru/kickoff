extern crate image;
extern crate rusttype;

pub struct Font<'a> {
    pub font: rusttype::Font<'a>,
    pub scale: rusttype::Scale,
}

impl Font<'_> {
    pub fn render(&self, text: &str, color: (u8, u8, u8)) -> image::RgbaImage {
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

        let mut image = image::RgbaImage::new(glyphs_width, glyphs_height);
        for glyph in glyphs {
            if let Some(bounding_box) = glyph.pixel_bounding_box() {
                glyph.draw(|x, y, v| {
                    let x = x + bounding_box.min.x as u32;
                    let y = y + bounding_box.min.y as u32;
                    if x < glyphs_width && y < glyphs_height {
                        image.put_pixel(
                            x,
                            y,
                            image::Rgba([color.0, color.1, color.2, (v * 255.) as u8]),
                        )
                    }
                });
            }
        }
        return image;
    }
}
