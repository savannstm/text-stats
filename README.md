# text-stats

Word and character counts for text in a lot of languages.

```rust
let counts = text_stats::count("Don't panic, it's 3.14 o'clock!");

assert_eq!(counts.words, 5);
assert_eq!(counts.characters, 31);
assert_eq!(counts.characters_no_whitespace, 27);
```

- **Words** are the word-like segments of the ICU word segmenter: letters, numbers and CJK ideographs/kana, never whitespace, punctuation or emoji. `don't` and `3.14` are one word, `well-known` is two.
- **Characters** are extended grapheme clusters (a letter with its accent, a flag or a family emoji is one character), spaces included. `characters_no_whitespace` leaves out the clusters that start with whitespace.
- No language has to be given; mixed-script text works.

## Features

| Feature      | Default | What it does                                                                                                                           |
| ------------ | ------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `dictionary` | yes     | Splits Chinese, Japanese, Thai, Lao, Khmer and Burmese into words with dictionaries. Without it a run of such text counts as one word. |
| `lstm`       | no      | Adds LSTM models for Thai, Lao, Khmer and Burmese (better than dictionaries there); Chinese and Japanese keep using the dictionary.    |

Both add data to the binary, which is why they can be turned off for projects that only count space-delimited languages.

## Reusing the segmenters

`count` uses a shared `Counter`. Build your own with `Counter::new()` if you want to control when the data is loaded; a `Counter` is `Send + Sync` and cheap to call.

Counts of several texts add up (`Counts` implements `Add` and `Sum`).
