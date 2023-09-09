# verses

Verses is a TUI tool to view synchronized Spotify lyrics.

![Preview GIF](./assets/showcase.gif)

## Installation

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

TODO: aur package?

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

## Windows support
Windows was not tested at all, and while it should run well, I do not guarantee flawless performance