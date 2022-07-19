mpris-notifier
================

Simple version: Shows desktop notifications for media/music track changes.

Technical version: Dependency-light, highly-customizable, XDG desktop notification generator for 🎵 🎹 MPRIS media status changes 💿.

![Screenshot of mpris-notifier being displayed with mako](https://raw.githubusercontent.com/l1na-forever/mpris-notifier/mainline/assets/screenshot1.png)
*Screenshot of mpris-notifier ([cfg](https://gist.github.com/l1na-forever/97edb7424183c2b376c133d2c4f72ca3)) being displayed with [mako](https://github.com/emersion/mako) ([cfg](https://gist.github.com/l1na-forever/432178c8455ca065e661d96ed4b61a8b))*

## Features:

* **Dependency-light**: *mpris-notifier* tries to keep a fairly minimal dependency surface, where each dependency is [documented and justified in the Cargo.toml](https://github.com/l1na-forever/mpris-notifier/blob/mainline/Cargo.toml).
* **Compatibility**: *mpris-notifier* implements the [MPRIS2 Player specification](https://specifications.freedesktop.org/mpris-spec/latest/) as well as the [XDG Desktop Notifications specification](https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html), making it highly compatible among both media clients as well as notification daemons.
* **Customizability**: Most aspects of *mpris-notifier*'s behavior can be customized, particularly the format of the generated messages.

*mpris-notifier* is particularly useful for when..

* .. you run a headless music client, or a music client that doesn't natively emit desktop notifications (works great with [spotifyd](https://github.com/Spotifyd/spotifyd)!)
* .. you often switch between music clients, or run multiple music clients at once, and want consistent notifications without extra configuration (the same way [sxhkd](https://wiki.archlinux.org/title/Sxhkd) lets you reuse a hotkey configuration among multiple DEs)

## Installation

To install via [Cargo](https://doc.rust-lang.org/cargo/):

    cargo install mpris-notifier

To install from source, first, [install Rust](https:://rustup.rs/), and then run:

    git clone github.com/l1na-forever/mpris-notifier
    cd mpris-notifier
    cargo install --path .

**Binary releases can be found on the [Releases page](https://github.com/l1na-forever/mpris-notifier/releases/).** To install a binary release, simply copy the executable to wherever's convenient (such as `/usr/local/bin`, or `~/.local/bin`), and run it!

## Usage

Typically, *mpris-notifier* is added to your desktop environment's start script (such as your `~/.xprofile`, or your window manager's configuration file). Add a line to run `mpris-notifier` somewhere in the script:

    mpris-notifier &

Upon first run, a configuration file with default values is generated at `~/.config/mpris-notifier/config.toml`. After customizing the configuration, restart `mpris-notifier`:

    pkill mpris-notifier; mpris-notifier &

Configuration keys are as follows:

* `subject_format`: Format string for the notification subject text.
* `body_format`: Format string for the notification message text.
* `join_string`: For fields including multiple entities (such as "artists"), this determines which character is used to join the strings.
* `enable_album_art`: Enable album artwork fetch. When enabled, album artwork will appear alongside the album art, provided that the art fetch completes within the deadline.
* `album_art_deadline`: The deadline, in milliseconds, before which the album art fetch must complete, else the notification will be sent without artwork.

## Troubleshooting

**mpris-notifier fails to start**

Make sure that the environment you're starting `mpris-notifier` from has a session D-Bus available. You can verify this by running `dbus-monitor` from the same context as with `mpris-notifier`.

**mpris-notifier doesn't emit notifications**

First, verify that your notification daemon is working as expected by sending a notification from the same environment as `mpris-notifier`:

    notify-send test # or dunstify test

Next, verify that MPRIS player properties signals are being emitted. Monitor the session D-Bus for the MPRIS signal, ensure lines are logged as the track changes:

    dbus-monitor | grep PlaybackStatus

If both above steps succeed but *mpris-notifier* isn't emitting notifications, [please file an issue](https://github.com/l1na-forever/mpris-notifier/issues/new/choose) 🛠️ 🩹.

## Status

`mpris-notifier` is mostly complete for my purposes, though I'll add small features as I need them. If there's an enhancement you'd like to see implemented (or would like to contribute 🥺), [please file an issue](https://github.com/l1na-forever/mpris-notifier/issues/new/choose).

## Licence

Copyright © 2022 Lina

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
