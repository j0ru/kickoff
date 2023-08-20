use image::Rgba;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<css_color::Rgba> for Color {
    fn from(c: css_color::Rgba) -> Self {
        Self(
            (c.red * 255. * c.alpha) as u8,
            (c.green * 255. * c.alpha) as u8,
            (c.blue * 255. * c.alpha) as u8,
            (c.alpha * 255.) as u8,
        )
    }
}

impl Color {
    pub const fn to_rgba(&self) -> Rgba<u8> {
        Rgba([self.0, self.1, self.2, self.3])
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ColorVisitor)
    }
}

struct ColorVisitor;

impl<'de> Visitor<'de> for ColorVisitor {
    type Value = Color;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex rgb or rgba color value")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let c = css_color::Rgba::from_str(value);
        c.map_or_else(|_| Err(de::Error::custom("")), |c| Ok(Color::from(c)))
    }
}
