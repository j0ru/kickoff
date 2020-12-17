extern crate clap;

use clap::{Arg, App, crate_version, crate_authors};

fn num_validator (num_str: String) -> Result<(), String> {
  match num_str.parse::<u32>() {
    Ok(_) => Ok(()),
    Err(e) => Err(e.to_string()),
  }
}

fn hex_validator (hex_str: String) -> Result<(), String> {
  match hex_str.parse::<css_color::Rgba>() {
    Ok(_) => Ok(()),
    Err(_) => Err("color parsing error".to_string()),
  }
}

pub fn build_cli() -> App<'static, 'static> {
  App::new("Kickoff")
    .version(crate_version!())
    .author(crate_authors!())
    .about("Minimal program launcher, focused on usability and speed")
    .arg(Arg::with_name("width")
      .short("w")
      .long("width")
      .value_name("PIXEL")
      .validator(num_validator)
      .help("Set window width"))
    .arg(Arg::with_name("heigth")
      .short("h")
      .long("heigth")
      .value_name("PIXEL")
      .validator(num_validator)
      .help("Set window heigth"))
    .arg(Arg::with_name("background")
      .long("background")
      .value_name("COLOR")
      .validator(hex_validator)
      .help("Background color"))
}