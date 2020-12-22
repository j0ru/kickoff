use image::Rgba;

pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<css_color::Rgba> for Color {
    fn from(c: css_color::Rgba) -> Self {
        Color(
            (c.red * 255.) as u8,
            (c.green * 255.) as u8,
            (c.blue * 255.) as u8,
            (c.alpha * 255.) as u8,
        )
    }
}

impl Color {
    pub fn to_rgba(&self) -> Rgba<u8> {
        Rgba([self.0, self.1, self.2, self.3])
    }
}
