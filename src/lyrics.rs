//! The word-timed lyrics, parsed from the enhanced LRC the user supplied
//! (`[line start]<word start>word<word start>word...`).

/// How long the last word of a line is held when the next line is far away.
const LAST_WORD_HOLD: f32 = 1.2;
/// A word may not start later than this before the next line (the LRC has
/// `起きる` at 44.49 while the next line starts at 44.35).
const MIN_GAP_TO_NEXT_LINE: f32 = 0.6;

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start: f32,
    pub end: f32,
    /// The LRC had a trailing space after this word: a visual phrase break.
    pub break_after: bool,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub start: f32,
    pub words: Vec<Word>,
}

impl Line {
    pub fn end(&self) -> f32 {
        self.words.last().map_or(self.start, |w| w.end)
    }
}

fn parse_time(s: &str) -> Option<f32> {
    let (m, sec) = s.split_once(':')?;
    Some(m.trim().parse::<f32>().ok()? * 60.0 + sec.trim().parse::<f32>().ok()?)
}

pub fn parse(src: &str) -> Vec<Line> {
    let mut raw: Vec<(f32, Vec<(f32, String)>)> = Vec::new();
    for line in src.lines().map(str::trim).filter(|l| l.starts_with('[')) {
        let Some((stamp, rest)) = line[1..].split_once(']') else { continue };
        let Some(start) = parse_time(stamp) else { continue };
        let words = rest
            .split('<')
            .filter_map(|piece| {
                let (t, text) = piece.split_once('>')?;
                Some((parse_time(t)?, text.to_string()))
            })
            .filter(|(_, text)| !text.trim().is_empty())
            .collect();
        raw.push((start, words));
    }

    let mut lines = Vec::with_capacity(raw.len());
    for i in 0..raw.len() {
        let next_line = raw.get(i + 1).map(|(s, _)| *s);
        let limit = next_line.map_or(f32::MAX, |n| n - MIN_GAP_TO_NEXT_LINE);
        let (start, words) = &raw[i];
        let n = words.len();
        let mut out: Vec<Word> = Vec::with_capacity(n);
        for (k, (ws, text)) in words.iter().enumerate() {
            let ws = ws.min(limit);
            let end = match words.get(k + 1) {
                Some((next, _)) => next.min(limit),
                None => (ws + LAST_WORD_HOLD).min(next_line.map_or(f32::MAX, |n| n - 0.05)),
            };
            out.push(Word {
                break_after: text.ends_with(' '),
                text: text.trim().to_string(),
                start: ws,
                end: end.max(ws + 0.05),
            });
        }
        lines.push(Line { start: *start, words: out });
    }
    lines
}
