//! JavaScript-compatible handling of plain JSON values: string lengths in UTF-16 units (as `String.length`), numbers that compare
//! and print as JavaScript numbers do (`18` and `18.0` are one number, written `18`), and the control-character test the TS package uses.

use serde_json::{Map, Number, Value};

pub type Obj = Map<String, Value>;

/// `s.length` in JavaScript.
pub fn jlen(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(0, max)` in JavaScript (a surrogate pair at the cut is dropped whole rather than split).
pub fn bound(s: &str, max: usize) -> String {
    if jlen(s) <= max {
        return s.to_string();
    }
    let mut n = 0;
    let mut out = String::new();
    for c in s.chars() {
        let w = c.len_utf16();
        if n + w > max {
            break;
        }
        n += w;
        out.push(c);
    }
    out
}

/// `/[\u0000-\u001f\u007f-\u009f]/`.
pub fn is_control(c: char) -> bool {
    let u = c as u32;
    u <= 0x1f || (0x7f..=0x9f).contains(&u)
}

pub fn has_control(s: &str) -> bool {
    s.chars().any(is_control)
}

/// A finite f64 as a JSON number the way JavaScript writes it: whole numbers without a fraction.
pub fn num(x: f64) -> Value {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        if x == 0.0 {
            return Value::Number(Number::from(0i64));
        }
        return Value::Number(Number::from(x as i64));
    }
    Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
}

/// Rewrites every number in `v` to its canonical form (see `num`).
pub fn norm(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) => num(f),
            None => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
        x => x,
    }
}

/// `a === b` for plain values (numbers compared by value).
pub fn js_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// A finite number.
pub fn finite(v: &Value) -> Option<f64> {
    v.as_f64().filter(|f| f.is_finite())
}

/// `String(v)` for plain values.
pub fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        Value::Number(n) => js_number_text(n),
        other => compact(other),
    }
}

/// `String(x)` for a finite number (`Number::toString`, radix 10): the shortest digits that read back as `x`, laid out as
/// JavaScript does (`0.000001`, `1e-7`, `1e+21`, `123.456`, `-0` written `0`).
pub fn js_f64(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // `{:e}` gives the shortest round-trip digits: `d[.ddd]e<exp>`.
    // Accepted residual (Sol r2 #8): where two shortest decimals round-trip equally (beyond 15 significant digits), Rust and
    // JavaScript may pick different last digits. Schema numbers are small bounded values, so settings and export bytes never reach it.
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exp.parse::<i64>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let es = if e < 0 { format!("-{}", -e) } else { format!("+{e}") };
        if k == 1 {
            format!("{digits}e{es}")
        } else {
            format!("{}.{}e{es}", &digits[..1], &digits[1..])
        }
    };
    format!("{sign}{body}")
}

fn js_number_text(n: &Number) -> String {
    match n.as_f64() {
        Some(f) => js_f64(f),
        None => n.to_string(),
    }
}

/// A JSON string literal as `JSON.stringify` writes it (only `"`, `\\` and C0 controls escaped; `\u00xx` in lower case).
fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A canonical array index ("0", "17"; below 2^32 - 1): JavaScript lists such keys first, in ascending order.
fn array_index(k: &str) -> Option<u32> {
    if k.is_empty() || (k.len() > 1 && k.starts_with('0')) || !k.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    k.parse::<u64>().ok().filter(|n| *n < u32::MAX as u64).map(|n| n as u32)
}

/// An object's keys in JavaScript's property order: array indices ascending, then the rest as inserted.
fn js_key_order(o: &Obj) -> Vec<(&String, &Value)> {
    let mut idx: Vec<(u32, (&String, &Value))> = o.iter().filter_map(|(k, v)| array_index(k).map(|i| (i, (k, v)))).collect();
    idx.sort_by_key(|(i, _)| *i);
    let mut all: Vec<(&String, &Value)> = idx.into_iter().map(|(_, kv)| kv).collect();
    all.extend(o.iter().filter(|(k, _)| array_index(k).is_none()));
    all
}

fn write_value(out: &mut String, v: &Value, indent: Option<usize>, depth: usize) {
    let nl = |out: &mut String, d: usize| {
        if let Some(w) = indent {
            out.push('\n');
            out.push_str(&" ".repeat(w * d));
        }
    };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            // JSON.stringify writes a non-finite number as null; serde_json cannot hold one, so this is the finite case.
            out.push_str(&js_number_text(n))
        }
        Value::String(s) => write_str(out, s),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                write_value(out, x, indent, depth + 1);
            }
            nl(out, depth);
            out.push(']');
        }
        Value::Object(o) => {
            if o.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, x)) in js_key_order(o).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                write_str(out, k);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_value(out, x, indent, depth + 1);
            }
            nl(out, depth);
            out.push('}');
        }
    }
}

/// `JSON.stringify(v)`.
pub fn compact(v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, v, None, 0);
    s
}

/// `JSON.stringify(v, null, 2) + "\n"`.
pub fn pretty(v: &Value) -> String {
    let mut s = String::new();
    write_value(&mut s, v, Some(2), 0);
    s.push('\n');
    s
}

/// ms since 1970 as `new Date(ms).toISOString()`.
pub fn iso(ms: f64) -> String {
    use chrono::{TimeZone, Utc};
    match Utc.timestamp_millis_opt(ms as i64).single() {
        Some(d) => d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        None => "?".into(),
    }
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Random lower-case hex of `bytes` bytes.
pub fn rand_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::getrandom(&mut buf).is_err() {
        let t = now_ms() as u64 ^ (std::process::id() as u64) << 20;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (t >> ((i % 8) * 8)) as u8 ^ i as u8;
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// `crypto.randomUUID()` (v4).
pub fn uuid() -> String {
    let h = rand_hex(16);
    let mut b: Vec<char> = h.chars().collect();
    b[12] = '4';
    let v = u8::from_str_radix(&b[16].to_string(), 16).unwrap_or(0);
    b[16] = std::char::from_digit(((v & 0x3) | 0x8) as u32, 16).unwrap_or('8');
    let s: String = b.into_iter().collect();
    format!("{}-{}-{}-{}-{}", &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_print_and_compare_as_javascript() {
        assert_eq!(compact(&num(18.0)), "18");
        assert_eq!(js_f64(0.000001), "0.000001");
        assert_eq!(js_f64(1e-7), "1e-7");
        assert_eq!(js_f64(1e21), "1e+21");
        assert_eq!(js_f64(-0.0), "0");
        assert_eq!(num(1.5).to_string(), "1.5");
        assert!(js_eq(&json!(18), &json!(18.0)));
        assert_eq!(norm(json!({"a": 2.0})).to_string(), r#"{"a":2}"#);
        assert_eq!(js_string(&json!(16)), "16");
    }

    #[test]
    fn lengths_are_utf16() {
        assert_eq!(jlen("⌘K"), 2);
        assert_eq!(jlen("😀"), 2);
        assert_eq!(bound("abc", 2), "ab");
        assert_eq!(iso(0.0), "1970-01-01T00:00:00.000Z");
        assert_eq!(uuid().len(), 36);
    }
}
