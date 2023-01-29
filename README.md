<h1 align="center">
  <img src="assets/logo.svg" width="300" height="300" alt="logo">
</h1>

Kickoff is heavily inspired by rofi, but not without changes made.
Like many programs, kickoff was born from an itch that no existing program seemed to relieve and my desire to learn a lower level programming language.

[![AUR version](https://img.shields.io/aur/version/kickoff?label=kickoff&logo=arch-linux&style=for-the-badge)](https://aur.archlinux.org/packages/kickoff/)
[![Crates Version](https://img.shields.io/crates/v/kickoff?style=for-the-badge)](https://crates.io/crates/kickoff)

![screenshot](assets/screenshot.png)

## Install

#### Arch Linux

Use your favourite AUR manager, i.e. [blinky](https://github.com/cherti/blinky/):

```bash
blinky -S kickoff
```

#### Cargo

```bash
cargo install kickoff
```

## How does it search

All programs found in $PATH are included in the search results.
This can include your own additions to $PATH as long as they
are done before you launch kickoff or the program that launches kickoff
(i.e. your window manager)

This list is then combined with your previous searches and sorted by amount of usage
and if it fits the search query.

## Features

- Wayland native (only wlroots based compositors though)
- Fuzzy search
- Fast and snappy
- Remembers often used applications
- Argument support for launched programs
- Paste support
- Build list from stdin
- Launch .desktop files [kickoff-dot-desktop](https://github.com/j0ru/kickoff-dot-desktop)

## Configuration

A default configuration will be placed at `$XDG_CONFIG_HOME/kickoff/config.toml`
or can be found [here](https://github.com/j0ru/kickoff/blob/main/assets/default_config.toml).