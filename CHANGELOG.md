# Changelog

## [0.3.0] - 2026-06-13

## Added
- "Play as" context menu entry to play a song with the demuxer of choice

## Changed
- When opening a music folder, songs are now scanned on a background thread for less delay
- Better error messages when mpv terminates with non-zero exit status
- mpv console now uses pre-defined commands (only a single `lavfi` command is implemented right now)
- Display a progress bar rather than a slider when source is non-seekable

## Fixed
- Fallback fonts now also apply for monospace
- Improved unicode handling in the built-in terminal
- Avoid leaving behing zombie processes
- mpv or the demuxer could block music playback if too much is written to the terminal at once
- Fixed some odd playlist behaviors

