use crossterm::event::{KeyEvent, KeyModifiers};

/// Mapování klávesových zkratek pro přepínání svazků (disky)
/// Podporuje číslice, české znaky a shift+number znaky
pub fn volume_shortcut_index(ch: char) -> Option<usize> {
    match ch {
        '1' | '+' | '!' => Some(0),      // C:
        '2' | 'ě' | 'Ě' | '@' => Some(1), // D:
        '3' | 'š' | 'Š' | '#' => Some(2), // E:
        '4' | 'č' | 'Č' | '$' => Some(3), // F:
        '5' | 'ř' | 'Ř' | '%' => Some(4), // G:
        '6' | 'ž' | 'Ž' | '^' => Some(5), // H:
        '7' | 'ý' | 'Ý' | '&' => Some(6), // I:
        '8' | 'á' | 'Á' | '*' => Some(7), // J:
        '9' | 'í' | 'Í' | '(' => Some(8), // K:
        _ => None,
    }
}

/// Zjištění, zda je stisknuta klávesa s Shift
pub fn is_shift_pressed(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT)
}

/// Detekce shift-filtrujících znaků (alfanumerické, podtržítko, pomlčka, tečka)
pub fn is_filter_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_numeric_root_shortcuts() {
        assert_eq!(volume_shortcut_index('1'), Some(0));
        assert_eq!(volume_shortcut_index('4'), Some(3));
        assert_eq!(volume_shortcut_index('9'), Some(8));
    }

    #[test]
    fn maps_czech_keyboard_shortcuts() {
        assert_eq!(volume_shortcut_index('ě'), Some(1));
        assert_eq!(volume_shortcut_index('š'), Some(2));
        assert_eq!(volume_shortcut_index('č'), Some(3));
    }

    #[test]
    fn accepts_filter_chars_only() {
        assert!(is_filter_char('a'));
        assert!(is_filter_char('_'));
        assert!(is_filter_char('.'));
        assert!(!is_filter_char('/'));
        assert!(!is_filter_char(' '));
    }
}
