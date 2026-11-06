# Changelog

## [0.4.0] - 2026-11-06

## Added
- Update egui to 0.37, featuring color emoji and fallback font detection
- Forward hardware volume up and volume down keys to mpv
- Ui scaling (zoom in/out)
- Remember window size on exit

## Fixed
- Properly track whether mpv is paused or not

## Changed
- Put preferences into a new "Preferences" submenu
- Video toggle checkbox is now a graphical button
- Mpv socket is put in `$XDG_RUNTIME_DIR` rather than `/tmp`

## [0.3.1] - 2026-06-18

## Fixed
- Sometimes mpvfrog wouldn't realize mpv has exited
- Sometimes mpvfrog would read incomplete json responses from mpv

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

