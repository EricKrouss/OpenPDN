//! Live canvas text state. Word navigation/deletion follows TextTool 5.1.12.
//! Indices are UTF-8 boundaries, keeping supplementary Unicode characters intact.
use std::ops::Range;
use unicode_general_category::{GeneralCategory as G, get_general_category};

#[derive(Clone, Debug, Default)]
pub struct CanvasText {
    text: String,
    cursor: usize,
    composition: Option<Range<usize>>,
}
#[derive(Clone, Copy, Debug)]
pub enum TextMotion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    Start,
    Finish,
}
fn letter_or_digit(c: char) -> bool {
    matches!(
        get_general_category(c),
        G::UppercaseLetter
            | G::LowercaseLetter
            | G::TitlecaseLetter
            | G::ModifierLetter
            | G::OtherLetter
            | G::DecimalNumber
    )
}
fn punctuation(c: char) -> bool {
    matches!(
        get_general_category(c),
        G::ConnectorPunctuation
            | G::DashPunctuation
            | G::OpenPunctuation
            | G::ClosePunctuation
            | G::InitialPunctuation
            | G::FinalPunctuation
            | G::OtherPunctuation
    )
}
fn previous(text: &str, at: usize) -> usize {
    text[..at].char_indices().next_back().map_or(0, |(i, _)| i)
}
fn next(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}
impl CanvasText {
    pub fn new(text: String) -> anyhow::Result<Self> {
        anyhow::ensure!(text.len() <= 65536, "Text is limited to 64 KiB per edit");
        let cursor = text.len();
        Ok(Self {
            text,
            cursor,
            composition: None,
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn composing(&self) -> bool {
        self.composition.is_some()
    }
    pub fn line_cursor(&self) -> (usize, usize) {
        let line = self.text[..self.cursor]
            .bytes()
            .filter(|&b| b == b'\n')
            .count();
        (line, self.cursor - self.line_start())
    }
    fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1)
    }
    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |i| self.cursor + i)
    }
    pub fn set_cursor(&mut self, line: usize, offset: usize) {
        self.cancel_composition();
        let mut start = 0;
        for _ in 0..line {
            let Some(i) = self.text[start..].find('\n') else {
                break;
            };
            start += i + 1;
        }
        let end = self.text[start..]
            .find('\n')
            .map_or(self.text.len(), |i| start + i);
        let mut at = start.saturating_add(offset).min(end);
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        self.cursor = at;
    }
    /// Paste uses LF internally. Control characters other than line breaks are
    /// ignored by the original TextTool, including CR in a CRLF clipboard string.
    pub fn insert(&mut self, value: &str) {
        self.cancel_composition();
        self.insert_clean(value);
    }
    fn insert_clean(&mut self, value: &str) {
        let clean: String = value
            .chars()
            .filter(|&c| c == '\n' || !c.is_control())
            .collect();
        if self.text.len().saturating_add(clean.len()) > 65536 {
            return;
        }
        self.text.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }
    pub fn motion(&mut self, motion: TextMotion) {
        self.cancel_composition();
        let start = self.line_start();
        let end = self.line_end();
        self.cursor = match motion {
            TextMotion::Left => previous(&self.text, self.cursor),
            TextMotion::Right => next(&self.text, self.cursor),
            TextMotion::Home => start,
            TextMotion::End => end,
            TextMotion::Start => 0,
            TextMotion::Finish => self.text.len(),
            TextMotion::WordLeft if self.cursor == start => previous(&self.text, self.cursor),
            TextMotion::WordRight if self.cursor == end => next(&self.text, self.cursor),
            TextMotion::WordLeft => self.word_left(start),
            TextMotion::WordRight => self.word_right(end),
        };
    }
    fn word_left(&self, start: usize) -> usize {
        let mut at = self.cursor;
        while at > start && self.text[..at].chars().next_back().unwrap().is_whitespace() {
            at = previous(&self.text, at);
        }
        if at > start {
            let c = self.text[..at].chars().next_back().unwrap();
            let class: fn(char) -> bool = if letter_or_digit(c) {
                letter_or_digit
            } else if punctuation(c) {
                punctuation
            } else {
                |_| false
            };
            at = previous(&self.text, at);
            while at > start && class(self.text[..at].chars().next_back().unwrap()) {
                at = previous(&self.text, at);
            }
        }
        at
    }
    fn word_right(&self, end: usize) -> usize {
        let mut at = self.cursor;
        let c = self.text[at..].chars().next().unwrap();
        let class: fn(char) -> bool = if letter_or_digit(c) {
            letter_or_digit
        } else if at > self.line_start() && punctuation(c) {
            punctuation
        } else {
            |_| false
        };
        at = next(&self.text, at);
        while at < end && class(self.text[at..].chars().next().unwrap()) {
            at = next(&self.text, at);
        }
        while at < end && self.text[at..].chars().next().unwrap().is_whitespace() {
            at = next(&self.text, at);
        }
        at
    }
    pub fn delete(&mut self, backwards: bool, word: bool) {
        self.cancel_composition();
        let original = self.cursor;
        let start = self.line_start();
        let end = self.line_end();
        let target = if backwards {
            if !word || original == start {
                previous(&self.text, original)
            } else {
                let flag = !self.text[..original]
                    .chars()
                    .next_back()
                    .unwrap()
                    .is_whitespace()
                    && original != end
                    && !letter_or_digit(self.text[original..].chars().next().unwrap());
                let mut at = self.word_left(start);
                if flag {
                    while at > start && self.text[..at].chars().next_back().unwrap().is_whitespace()
                    {
                        at = previous(&self.text, at);
                    }
                }
                at
            }
        } else if !word || original == end {
            next(&self.text, original)
        } else {
            let flag = original == start
                || (!letter_or_digit(self.text[..original].chars().next_back().unwrap())
                    && !self.text[original..]
                        .chars()
                        .next()
                        .unwrap()
                        .is_whitespace());
            let mut at = original;
            while at < end && self.text[at..].chars().next().unwrap().is_whitespace() {
                at = next(&self.text, at);
            }
            if at < end {
                let c = self.text[at..].chars().next().unwrap();
                let class: fn(char) -> bool = if letter_or_digit(c) {
                    letter_or_digit
                } else if punctuation(c) {
                    punctuation
                } else {
                    |_| false
                };
                at = next(&self.text, at);
                while at < end && class(self.text[at..].chars().next().unwrap()) {
                    at = next(&self.text, at);
                }
            }
            if flag {
                while at < end && self.text[at..].chars().next().unwrap().is_whitespace() {
                    at = next(&self.text, at);
                }
            }
            at
        };
        let range = target.min(original)..target.max(original);
        self.cursor = range.start;
        self.text.replace_range(range, "");
    }
    /// Preedit replaces only the previous candidate; committed text is never
    /// appended twice. The candidate remains outside undo until acceptance.
    pub fn preedit(&mut self, text: &str, active: Option<Range<usize>>) {
        self.cancel_composition();
        if text.is_empty() {
            return;
        }
        let start = self.cursor;
        self.insert_clean(text);
        let end = self.cursor;
        self.composition = Some(start..end);
        let chars = active.map_or(text.chars().count(), |r| r.end);
        self.cursor = start
            + self.text[start..end]
                .char_indices()
                .nth(chars)
                .map_or(end - start, |(i, _)| i);
    }
    pub fn commit(&mut self, value: &str) {
        self.cancel_composition();
        self.insert_clean(value);
    }
    pub fn cancel_composition(&mut self) {
        if let Some(range) = self.composition.take() {
            self.cursor = range.start;
            self.text.replace_range(range, "");
        }
    }
    pub fn delete_surrounding(&mut self, before: usize, after: usize) {
        self.cancel_composition();
        let mut start = self.cursor;
        let mut end = self.cursor;
        for _ in 0..before.min(self.text.len()) {
            start = previous(&self.text, start);
        }
        for _ in 0..after.min(self.text.len()) {
            end = next(&self.text, end);
        }
        self.text.replace_range(start..end, "");
        self.cursor = start;
    }
}
