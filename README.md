# mpvfrog 🐸
Simple GUI music player for Linux, wrapping mpv

![Image](https://github.com/user-attachments/assets/e32bb4a7-4d29-44c1-9fb5-475a2eb21ac7)

Currently it reads songs (recursively) from a single folder, but this might change in the future.

## Features

### Custom demuxers

Define custom demuxer commands for file types that mpv does not support natively.
Custom demuxers write to stdout, and will be piped to mpv stdin.

The demuxer can even be controlled through the Demuxer tab in the UI.
Key events are forwarded to it. This also applies to the Mpv tab by the way.

![Image](https://github.com/user-attachments/assets/95a7ab4d-a1c8-4806-b521-91d3e897298e)

### Custom tray popup

It provides a volume slider, and even forwards key events to mpv.

![Image](https://github.com/user-attachments/assets/8697c79d-7eca-4e34-a940-5110206983fe)

## Installing

If you have Rust 1.86 or later, you can do `cargo install --git https://github.com/crumblingstatue/mpvfrog.git`.

Otherwise, check out the [Releases](<https://github.com/crumblingstatue/mpvfrog/releases>).

## FAQ

### Why?
I really like mpv.
It supports all the formats and has all the features I want, including speeding up and slowing down music on the fly.
(I am a musical pervert who likes to do that.)
All I need is a nice gui for picking songs with a single click.

### Why not libmpv?
I was mostly just curious about how far I could go just wrapping the system mpv process.
I might use libmpv in the future, but for my personal needs, this is sufficient.
