# verses

Verses is a TUI tool to view synchronized Spotify lyrics.

![Preview GIF](./assets/showcase.gif)

## Installation

### AUR (for Arch users)
Verses is available on AUR under `verses-bin`

```
yay -S verses-bin
```

### With cargo:
```sh
cargo install verses
```

### From source:

```
git clone https://github.com/Maxuss/verses
cd verses
cargo install --path .
```

## Prerequesities

To track spotify stats you will have to create your own Spotify developer app [here](https://developer.spotify.com/dashboard/create).

Run `verses` for the first time and it will prompt you for your
Client ID. You can get it in the *Settings* section of your app dashboard. Do not confuse it with client secret!

After that, you can run verses.

## Controls

* `q` - quit
* `a` - toggle auto-scrolling
* `j` | `down key` - scroll down
* `k` | `up key` - scroll up
* `r` - reset scroll position

## Config

Config file is located at `$HOME/.config/verses/config.toml`

Each TOML section can be included in a separate file, just specify it using the `include` field.

For example:

```toml
# config.toml

[theme]
include = "themes/catppuccin.toml"

# themes/catppuccin.toml
[borders]
# configuration there...

[lyrics]
# ...

[progress_bar]
# ...
```

Config has certain special value types:

### general.display.*_format

These are formatting strings using the Handlebars syntax, specifically [the Rust implementation](https://github.com/sunng87/handlebars-rust).
Available variables are listend in the example config. There is also an utility function `join` that allows to join a list of strings separated by a comma.

### Colors

Colors can either be represented the [Ratatui stringified way](https://docs.rs/ratatui/latest/ratatui/style/enum.Color.html) or as a hex RGB value, prefixed with `#`

### Border styles

These are enum variants. You can see [all variants here](https://docs.rs/ratatui/latest/ratatui/widgets/block/enum.BorderType.html)

## Windows support
Windows was not tested at all, and while it should run well, I do not guarantee flawless performance