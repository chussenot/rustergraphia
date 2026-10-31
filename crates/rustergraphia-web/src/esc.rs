//! Text building blocks shared by every page: Erubi's escaping (what Ruby's
//! `<%= %>` does), JSON string generation matching Ruby's `JSON.generate`,
//! and Ruby's number-to-text forms (`%04d`, `Float#round(1).to_s`).

/// Erubi / `Rack::Utils.escape_html` escaping: `& < > " '` and nothing else
/// (`/` is left alone, unlike many templating defaults).
pub fn h(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// A JSON string literal as Ruby's `JSON.generate` writes it: `"` and `\`
/// escaped, control characters as `\uXXXX` (or the short forms), UTF-8 and
/// `/` left as they are.
pub fn json_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Ruby's `format('%04d', n)`.
pub fn pad4(n: u32) -> String {
    format!("{n:04}")
}

/// Ruby's `Float#round(1)`: round half away from zero at one decimal, with
/// MRI's representation correction.
pub fn round1(x: f64) -> f64 {
    let mut f = (x * 10.0).round();
    if x > 0.0 {
        if (f + 0.5) / 10.0 <= x {
            f += 1.0;
        }
    } else if (f - 0.5) / 10.0 >= x {
        f -= 1.0;
    }
    f / 10.0
}

/// A float written as Ruby writes one (`50.0`, `8.9`, `-0.0`): the shortest
/// representation, but integral values keep one decimal.
pub fn ruby_float(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// `Float#round(1)` then `to_s`, the pair the figures and the tray print with.
pub fn round1_s(value: f64) -> String {
    ruby_float(round1(value))
}
