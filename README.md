# A simple and uncomplicated program launcher for wayland

Kickoff is heavily inspired by rofi, but not without changes made.
Like many programs, kickoff was born from an itch that no existing program seemed to relieve and my desire to learn a lower level programming language.

## How does it search
At the moment the path is read and non-recursively filtered for executable files. Other locations are still up for discussion.

## State
This project is still in heavy development and code quality as well as test coverage are in dire need of improvement. But it is usable and I will try not to break *too* much between releases ;-)

## Features
* Wayland compatible (only wlroots based compositors though)
* Fuzzy search
* Fast and snappy (hopefully, improvements planned)
* Remembers often used applications
* Argument support for launched programs
* Paste support

## Configuration
A default configuration will be placed at `$XDG_CONFIG_HOME/kickoff/kickoff.toml`

| Key | Value |
| --- | --- |
| color_background | Background color |
| color_text | Default color for search results |
| color_text_selected | Color for the currently selected result |
| color_text_query | Color for the search query |
| color_prompt | Color for the prompt |
| prompt | Characters shown bevor the query |
| padding | Space between window border and content, does not affect background |
| font | Font used to render text, has to be ttf or otf |
| font_size | Font size |


## Roadmap (very incomplete and in no special order)
* UX improvements
* Extended configurability
* Testing and documentation
* Rewrite the config loading