//! Terminal output: colors, aligned tables and relative times.

use chrono::{DateTime, Utc};
use std::io::IsTerminal;

/// Colors only when writing to a terminal and `NO_COLOR` is not set.
pub fn colors() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn paint(code: &str, text: &str) -> String {
    if colors() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn bold(t: &str) -> String {
    paint("1", t)
}
pub fn dim(t: &str) -> String {
    paint("2", t)
}
pub fn green(t: &str) -> String {
    paint("32", t)
}
pub fn red(t: &str) -> String {
    paint("31", t)
}
pub fn magenta(t: &str) -> String {
    paint("35", t)
}
pub fn cyan(t: &str) -> String {
    paint("36", t)
}

/// Text in a label's own color (24-bit), falling back to plain text.
pub fn hex(color: &str, text: &str) -> String {
    match (colors(), u32::from_str_radix(color, 16)) {
        (true, Ok(n)) if color.len() == 6 => format!("\x1b[38;2;{};{};{}m{text}\x1b[0m", (n >> 16) & 255, (n >> 8) & 255, n & 255),
        _ => text.to_string(),
    }
}

/// "about 3 hours ago", like `gh`.
pub fn ago(when: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - when).num_seconds().max(0);
    let plural = |n: i64, unit: &str| format!("about {n} {unit}{} ago", if n == 1 { "" } else { "s" });
    match seconds {
        s if s < 60 => "less than a minute ago".to_string(),
        s if s < 3600 => plural(s / 60, "minute"),
        s if s < 86_400 => plural(s / 3600, "hour"),
        s if s < 86_400 * 30 => plural(s / 86_400, "day"),
        s if s < 86_400 * 365 => plural(s / (86_400 * 30), "month"),
        s => plural(s / (86_400 * 365), "year"),
    }
}

/// Width of text on screen, ignoring color escape sequences.
pub fn width(text: &str) -> usize {
    let mut count = 0;
    let mut escaped = false;
    for c in text.chars() {
        match (escaped, c) {
            (false, '\x1b') => escaped = true,
            (true, 'm') => escaped = false,
            (true, _) => {}
            (false, _) => count += 1,
        }
    }
    count
}

pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// Renders rows as aligned columns under dim upper-case headers.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(width(cell));
        }
    }
    let line = |cells: Vec<String>| {
        let last = cells.len() - 1;
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| if i == last { c.clone() } else { format!("{c}{}", " ".repeat(widths[i] - width(c))) })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = vec![line(headers.iter().map(|h| dim(h)).collect())];
    out.extend(rows.iter().map(|r| line(r.clone())));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn relative_times() {
        let now = Utc::now();
        assert_eq!(ago(now - Duration::seconds(5), now), "less than a minute ago");
        assert_eq!(ago(now - Duration::minutes(1), now), "about 1 minute ago");
        assert_eq!(ago(now - Duration::hours(5), now), "about 5 hours ago");
        assert_eq!(ago(now - Duration::days(45), now), "about 1 month ago");
        assert_eq!(ago(now + Duration::minutes(5), now), "less than a minute ago", "clock skew is not the future");
    }

    #[test]
    fn tables_align_and_ignore_color_codes() {
        assert_eq!(width("\x1b[32m#12\x1b[0m"), 3);
        let out = table(&["ID", "TITLE"], &[vec!["#1".into(), "One".into()], vec!["#123".into(), "Two".into()]]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], "#1    One");
        assert_eq!(lines[2], "#123  Two");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
    }
}
