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
    .arg(Arg::with_name("background-color")
      .long("background-color")
      .value_name("COLOR")
      .validator(hex_validator)
      .default_value("#222222aa")
      .help("Background color"))
}