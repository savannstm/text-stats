#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![doc = include_str!("../README.md")]

//! Word and character counts for text in any language.
//!
//! Counting is done the way translation tools do it, but with real Unicode segmentation instead of
//! splitting on spaces, so it also works for languages that do not use spaces:
//!
//! - **words** are the *word-like* segments of ICU4X's word segmenter: letters, numbers and CJK
//!   ideographs/kana, but not whitespace, punctuation or emoji. `don't` and `3.14` are one word,
//!   `well-known` is two. With the `dictionary` feature (on by default) Chinese, Japanese, Thai, Lao, Khmer
//!   and Burmese are split into real words; without it a run of such text is not split and counts as one word.
//! - **characters** are extended grapheme clusters, i.e. what a reader sees as one character (`e` plus a
//!   combining accent, or a family emoji, is one), not bytes or `char`s.
//! - **characters without whitespace** are the same clusters, minus the ones that start with whitespace.
//!
//! No language needs to be given: the segmenter works on mixed-script text directly.
//!
//! ```
//! use text_stats::count;
//!
//! let counts = count("Don't panic, it's 3.14 o'clock!");
//! assert_eq!(counts.words, 5);
//! assert_eq!(counts.characters, 31);
//! assert_eq!(counts.characters_no_whitespace, 27);
//! ```

use icu_segmenter::{
    GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, WordSegmenter, WordSegmenterBorrowed,
    options::WordBreakInvariantOptions,
};
use std::{
    iter::Sum,
    ops::{Add, AddAssign},
    sync::OnceLock,
};

/// Word and character counts of a text. Counts of several texts can be added.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    /// Word-like segments: letters, numbers, CJK ideographs and kana. Punctuation and whitespace are not words.
    pub words: u32,
    /// Extended grapheme clusters, whitespace included.
    pub characters: u32,
    /// Extended grapheme clusters that do not start with whitespace.
    pub characters_no_whitespace: u32,
}

impl Add for Counts {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            words: self.words.saturating_add(other.words),
            characters: self.characters.saturating_add(other.characters),
            characters_no_whitespace: self
                .characters_no_whitespace
                .saturating_add(other.characters_no_whitespace),
        }
    }
}

impl AddAssign for Counts {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sum for Counts {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

/// Counts words and characters. Building a `Counter` loads the segmentation data, so build one and reuse it;
/// [`count`] does that for you with a shared instance. A `Counter` can be used from many threads at once.
pub struct Counter {
    words: WordSegmenterBorrowed<'static>,
    graphemes: GraphemeClusterSegmenterBorrowed<'static>,
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

impl Counter {
    #[must_use]
    pub fn new() -> Self {
        let options = WordBreakInvariantOptions::default();

        #[cfg(feature = "lstm")]
        let words = WordSegmenter::new_auto(options);
        #[cfg(all(feature = "dictionary", not(feature = "lstm")))]
        let words = WordSegmenter::new_dictionary(options);
        #[cfg(not(any(feature = "dictionary", feature = "lstm")))]
        let words = WordSegmenter::new_for_non_complex_scripts(options);

        Self {
            words,
            graphemes: GraphemeClusterSegmenter::new(),
        }
    }

    /// Counts the words and characters of `text`. Line breaks (`\n`, `\r\n`) count as whitespace characters.
    #[must_use]
    pub fn count(&self, text: &str) -> Counts {
        let mut words = 0usize;
        let mut segments = self.words.segment_str(text);

        // The segment preceding each boundary is the one that is classified.
        while segments.next().is_some() {
            if segments.is_word_like() {
                words += 1;
            }
        }

        let mut characters = 0usize;
        let mut characters_no_whitespace = 0usize;
        let mut boundaries = self.graphemes.segment_str(text);
        let mut start = boundaries.next();

        for end in boundaries {
            if let Some(start) = start {
                characters += 1;

                // `start` is a boundary from the segmenter, so it is on a `char` boundary and the slice is not empty.
                if text[start..end]
                    .chars()
                    .next()
                    .is_some_and(|first| !first.is_whitespace())
                {
                    characters_no_whitespace += 1;
                }
            }

            start = Some(end);
        }

        Counts {
            words: u32::try_from(words).unwrap_or(u32::MAX),
            characters: u32::try_from(characters).unwrap_or(u32::MAX),
            characters_no_whitespace: u32::try_from(characters_no_whitespace).unwrap_or(u32::MAX),
        }
    }
}

/// Counts the words and characters of `text` with a shared [`Counter`].
#[must_use]
pub fn count(text: &str) -> Counts {
    static COUNTER: OnceLock<Counter> = OnceLock::new();

    COUNTER.get_or_init(Counter::new).count(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(words: u32, characters: u32, characters_no_whitespace: u32) -> Counts {
        Counts {
            words,
            characters,
            characters_no_whitespace,
        }
    }

    #[test]
    fn empty_and_whitespace() {
        assert_eq!(count(""), Counts::default());
        assert_eq!(count(" \t\r\n "), counts(0, 4, 0));
    }

    #[test]
    fn english() {
        assert_eq!(count("Hello, world!"), counts(2, 13, 12));
        // Apostrophes and decimal points stay inside a word, hyphens split.
        assert_eq!(count("don't 3.14 well-known"), counts(4, 21, 19));
    }

    #[test]
    fn other_space_delimited_scripts() {
        assert_eq!(count("Привет, мир!"), counts(2, 12, 11));
        assert_eq!(count("γειά σου κόσμε"), counts(3, 14, 12));
        assert_eq!(count("مرحبا بالعالم"), counts(2, 13, 12));
        assert_eq!(count("안녕하세요 세계"), counts(2, 8, 7));
    }

    #[test]
    fn grapheme_clusters_are_single_characters() {
        // `e` + combining acute, a flag (two regional indicators) and a ZWJ family emoji.
        assert_eq!(count("e\u{301}"), counts(1, 1, 1));
        assert_eq!(count("\u{1F1E9}\u{1F1EA}"), counts(0, 1, 1));
        assert_eq!(count("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"), counts(0, 1, 1));
    }

    #[test]
    fn crlf_is_one_whitespace_character() {
        assert_eq!(count("a\r\nb"), counts(2, 3, 2));
    }

    #[test]
    fn counts_add_up() {
        let total: Counts = ["one two", "three"].into_iter().map(count).sum();
        assert_eq!(total, counts(3, 12, 11));
    }

    #[cfg(not(any(feature = "dictionary", feature = "lstm")))]
    #[test]
    fn cjk_without_dictionary_is_one_word_per_run() {
        // Documented limitation: no segmentation model is available, so the whole run is one word.
        let result = count("今日は晴れ");
        assert_eq!(result.characters, 5);
        assert_eq!(result.words, 1);
    }

    #[cfg(any(feature = "dictionary", feature = "lstm"))]
    #[test]
    fn mixed_scripts_and_cjk_punctuation() {
        // Punctuation of any script is never a word.
        let result = count("\u{300C}Hello\u{3001}\u{4E16}\u{754C}\u{3002}\u{300D}");

        assert_eq!(result.characters, 11);
        assert!(result.words >= 2 && result.words <= 3, "{result:?}");
    }

    #[cfg(any(feature = "dictionary", feature = "lstm"))]
    #[test]
    fn japanese_and_chinese_are_segmented_into_words() {
        // "It is very nice weather today" - more than one word, fewer than the number of characters.
        let japanese = count(
            "\u{4ECA}\u{65E5}\u{306F}\u{3068}\u{3066}\u{3082}\u{3044}\u{3044}\u{5929}\u{6C17}\u{3067}\u{3059}\u{306D}",
        );
        assert!(
            japanese.words > 1 && japanese.words < japanese.characters,
            "{japanese:?}"
        );

        let chinese = count(
            "\u{4ECA}\u{5929}\u{5929}\u{6C14}\u{5F88}\u{597D}\u{FF0C}\u{6211}\u{4EEC}\u{53BB}\u{516C}\u{56ED}\u{6563}\\
             \
             u{6B65}",
        );
        assert!(chinese.words > 1 && chinese.words < chinese.characters, "{chinese:?}");
    }

    #[cfg(any(feature = "dictionary", feature = "lstm"))]
    #[test]
    fn thai_is_segmented_into_words() {
        // "Hello, world" in Thai, written without spaces.
        let thai =
            count("\u{0E2A}\u{0E27}\u{0E31}\u{0E2A}\u{0E14}\u{0E35}\u{0E0A}\u{0E32}\u{0E27}\u{0E42}\u{0E25}\u{0E01}");
        assert!(thai.words >= 2, "{thai:?}");
    }
}
