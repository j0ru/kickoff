<h1 align="center">
  <img src="assets/logo.svg" width="300" height="300" alt="logo">
</h1>

Kickoff is heavily inspired by rofi, but not without changes made.
Like many programs, kickoff was born from an itch that no existing program seemed to relieve and my desire to learn a lower level programming language.

[![AUR version](https://img.shields.io/aur/version/kickoff?label=kickoff&logo=arch-linux&style=for-the-badge)](https://aur.archlinux.org/packages/kickoff/)
[![AUR git version](https://img.shields.io/aur/version/kickoff-git?label=kickoff-git&logo=arch-linux&style=for-the-badge)](https://aur.archlinux.org/packages/kickoff-git/)
![screenshot](assets/screenshot.png)

## How does it search

At the moment the path is read and non-recursively filtered for executable files. Other locations are still up for discussion.

## State

This project is still in heavy development and code quality as well as test coverage are in dire need of improvement. But it is usable and I will try not to break _too_ much between releases ;-)

## Features

- Wayland native (only wlroots based compositors though)
- Fuzzy search
- Fast and snappy
- Remembers often used applications
- Argument support for launched programs
- Paste support

## Configuration

A default configuration will be placed at `$XDG_CONFIG_HOME/kickoff/config.toml`
or can be found [here](https://github.com/j0ru/kickoff/blob/main/assets/default_config.toml).

## Roadmap (very incomplete and in no special order)

- Speed improvements
- Testing and documentation
