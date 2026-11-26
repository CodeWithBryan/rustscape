//! `java.util.Scanner` (JDK 11) as `AIPlayer` reads its two name files with
//! it: `hasNextLine()`, `nextLine()` and `hasNext()` over the whole text of
//! a file, with the default delimiter (`\p{javaWhitespace}+`).
//!
//! A line is what `.*(\r\n|[\n\r  \u0085])|.+$` finds from the
//! position: the text up to the next separator (`\r\n` counts as one), or
//! the rest of the input when it is not empty. `hasNext()` looks for a token
//! without moving: a character that is not Java whitespace anywhere at or
//! after the position. A file that ends in blank lines therefore still has
//! lines after its last token.

/// A scanner over text already decoded with the JVM's default charset.
pub struct Scanner {
    text: Vec<char>,
    position: usize,
}

impl Scanner {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.chars().collect(),
            position: 0,
        }
    }

    /// `hasNextLine()`: anything is left to read.
    pub fn has_next_line(&self) -> bool {
        self.position < self.text.len()
    }

    /// `nextLine()`; `None` where the original throws NoSuchElementException.
    pub fn next_line(&mut self) -> Option<String> {
        if self.position >= self.text.len() {
            return None;
        }
        let start = self.position;
        let mut end = start;
        while end < self.text.len() && !is_line_separator(self.text[end]) {
            end += 1;
        }
        let line: String = self.text[start..end].iter().collect();
        self.position = match self.text.get(end) {
            Some('\r') if self.text.get(end + 1) == Some(&'\n') => end + 2,
            Some(_) => end + 1,
            None => end,
        };
        Some(line)
    }

    /// `hasNext()`: a token is left. The position does not move.
    pub fn has_next(&self) -> bool {
        self.text[self.position.min(self.text.len())..]
            .iter()
            .any(|c| !is_java_whitespace(*c))
    }
}

/// A character of the line pattern's separator class, which is also every
/// character `.` does not match.
fn is_line_separator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}

/// `Character.isWhitespace`: a space, line or paragraph separator that is not
/// a non-breaking space, or one of the listed controls.
pub fn is_java_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{000B}'
            | '\u{000C}'
            | '\r'
            | '\u{001C}'..='\u{001F}'
            | ' '
            | '\u{1680}'
            | '\u{2000}'..='\u{2006}'
            | '\u{2008}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{205F}'
            | '\u{3000}'
    )
}
