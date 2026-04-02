# Pruzkumnik

Pruzkumnik is a terminal file explorer written in Rust. It uses a three-pane layout:

- left pane: parent folder
- middle pane: current folder
- right pane: file preview

The program runs in the terminal, so it feels like a small GUI without leaving the console.

## Features

- Navigate folders with the keyboard
- Open folders with `Right Arrow` or `Enter`
- Open files in Notepad with `Enter`
- Show a help screen with `H`
- Copy, cut, paste, and delete files
- Ask for confirmation before delete
- Preview text files and ZIP archives
- Detect encrypted ZIP archives and show `Archive is encrypted.`
- Switch root drives with `1`, `2`, `3`, `4`
- Wrap preview text when the window size changes

## Controls

- `Up` / `Down` - move the selection in the current folder
- `Right Arrow` - enter a folder
- `Enter` - enter a folder or open a file in Notepad
- `Backspace` / `Left Arrow` - go to the parent folder
- `H` - show or hide help
- `C` - copy the selected item
- `X` - cut the selected item
- `V` - paste into the current folder
- `D` - delete the selected item
- `1`, `2`, `3`, `4` - switch to drives `C:`, `D:`, `E:`, `F:`
- `R` - refresh the current view
- `Q` - quit the program

## Rust structure

The project is split into small modules:

- `main.rs` - crate entry point
- `app.rs` - UI state, keyboard handling, and screen rendering
- `explorer_fs.rs` - directory listing and preview logic
- `actions.rs` - copy, cut, paste, delete, and Notepad opening
- `help.rs` - help text shown in the terminal

This is a good Rust pattern: keep the application state in one place and move file operations into separate modules.

## Build and run

```bash
cargo check
cargo run
```

## Learning terms

- `module` - a file or namespace that groups related code
- `struct` - a data type with named fields
- `enum` - a type with several possible states
- `borrow` - temporary read access without taking ownership
- `ownership` - Rust's rule for who owns a value
- `preview pane` - the right side panel that shows file content
- `clipboard` - temporary storage for copy and cut actions
- `modal` - a screen layer that asks for help or confirmation

## Notes

- The app is designed for Windows, because it opens files in Notepad and can switch drive roots.
- ZIP preview only reads the archive structure. It does not extract files.
