pub fn help_lines() -> &'static [&'static str] {
    &[
        "Help",
        "",
        "H = show or hide help",
        "Right arrow = enter a folder",
        "Enter = open folder or open file in Notepad",
        "C = copy",
        "V = paste",
        "D = delete",
        "X = cut",
        "1-9 = switch to volume 1-9 (first available drives)",
        "^ more above / v more below = scrolling indicators",
        "PageUp / PageDown = scroll the file list",
        "Backspace / Left arrow = go to parent folder",
        "R = refresh",
        "Q = quit",
        "",
        "Rust note: a module is a file that owns a part of the program.",
        "Functions stay smaller when you move file actions and UI text out of main app logic.",
    ]
}