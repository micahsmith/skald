use std::ffi::OsString;

use serde_json::{Map, Value};
use toon_format::EncodeOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Toon,
    Json,
}

impl Format {
    /// Reads `--json` from the raw arguments, so that usage errors from a failed parse honor it.
    pub fn from_args(args: &[OsString]) -> Self {
        let json = args
            .iter()
            .skip(1)
            .take_while(|arg| *arg != "--")
            .any(|arg| arg == "--json");
        if json { Self::Json } else { Self::Toon }
    }
}

#[derive(Debug)]
pub enum Output {
    Value {
        value: Map<String, Value>,
        help: Vec<String>,
        exit_code: u8,
    },
    /// Text written as is in either format, such as a rendered page.
    Raw(String),
}

impl Output {
    pub fn new(value: Map<String, Value>) -> Self {
        Self::Value {
            value,
            help: Vec::new(),
            exit_code: 0,
        }
    }

    /// Adds help lines to a value. Raw text has no place for them.
    pub fn with_help(mut self, lines: impl IntoIterator<Item = String>) -> Self {
        if let Self::Value { help, .. } = &mut self {
            help.extend(lines);
        }
        self
    }

    /// A non-zero code reports a failure whose output is not an error object.
    pub fn with_exit_code(mut self, code: u8) -> Self {
        if let Self::Value { exit_code, .. } = &mut self {
            *exit_code = code;
        }
        self
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Value { exit_code, .. } => *exit_code,
            Self::Raw(_) => 0,
        }
    }

    pub fn encode(self, format: Format) -> String {
        let (mut value, help) = match self {
            Self::Value { value, help, .. } => (value, help),
            Self::Raw(text) => return text,
        };
        if !help.is_empty() {
            value.insert("help".to_owned(), Value::from(help));
        }
        let value = Value::Object(value);
        let mut text = match format {
            Format::Json => value.to_string(),
            Format::Toon => toon_format::encode(&value, &EncodeOptions::default())
                .expect("a JSON object always encodes as TOON"),
        };
        text.push('\n');
        text
    }
}

const TRUNCATE_AT: usize = 500;

/// Cuts every string in `value` longer than 500 characters, and reports whether any was cut.
pub fn truncate_text(value: &mut Value) -> bool {
    match value {
        Value::String(text) => {
            let Some((end, _)) = text.char_indices().nth(TRUNCATE_AT) else {
                return false;
            };
            let total = text.chars().count();
            text.truncate(end);
            text.push_str(&format!(" ... (truncated, {total} chars total)"));
            true
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |cut, item| truncate_text(item) | cut),
        Value::Object(map) => map
            .values_mut()
            .fold(false, |cut, item| truncate_text(item) | cut),
        _ => false,
    }
}

/// Reports whether `text` is what `truncate_text` makes of a longer string.
pub fn is_truncated(text: &str) -> bool {
    let Some((end, _)) = text.char_indices().nth(TRUNCATE_AT) else {
        return false;
    };
    let Some(rest) = text[end..].strip_prefix(" ... (truncated, ") else {
        return false;
    };
    rest.strip_suffix(" chars total)")
        .and_then(|total| total.parse::<usize>().ok())
        .is_some_and(|total| total > TRUNCATE_AT && !rest.contains(['\n']))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn sample() -> Output {
        let Value::Object(value) = json!({"id": "d123", "name": "orders"}) else {
            unreachable!()
        };
        Output::new(value).with_help(["skald design get <id>".to_owned()])
    }

    #[test]
    fn json_flag_selects_json() {
        assert_eq!(Format::from_args(&args(&["skald"])), Format::Toon);
        assert_eq!(
            Format::from_args(&args(&["skald", "design", "list", "--json"])),
            Format::Json
        );
        assert_eq!(
            Format::from_args(&args(&["skald", "design", "get", "--", "--json"])),
            Format::Toon
        );
    }

    #[test]
    fn help_is_the_last_key() {
        let text = sample().encode(Format::Json);
        assert_eq!(
            text,
            "{\"id\":\"d123\",\"name\":\"orders\",\"help\":[\"skald design get <id>\"]}\n"
        );
    }

    #[test]
    fn toon_and_json_carry_the_same_content() {
        let toon = sample().encode(Format::Toon);
        let json = sample().encode(Format::Json);
        let from_toon: Value = toon_format::decode_default(&toon).unwrap();
        let from_json: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(from_toon, from_json);
        assert!(toon.starts_with("id: d123\n"), "toon:\n{toon}");
    }

    #[test]
    fn long_text_is_cut_with_a_size_hint() {
        let mut value = json!({"short": "abc", "list": [{"long": "é".repeat(501)}]});
        assert!(truncate_text(&mut value));
        assert_eq!(value["short"], "abc");
        assert_eq!(
            value["list"][0]["long"],
            format!("{} ... (truncated, 501 chars total)", "é".repeat(500))
        );

        let mut value = json!({"edge": "x".repeat(500), "number": 7});
        assert!(!truncate_text(&mut value));
        assert_eq!(value["edge"], "x".repeat(500));
    }

    #[test]
    fn truncated_text_is_recognized() {
        let mut value = json!({"long": "é".repeat(501)});
        truncate_text(&mut value);
        assert!(is_truncated(value["long"].as_str().unwrap()));
        assert!(!is_truncated(&"é".repeat(501)));
        assert!(!is_truncated(" ... (truncated, 600 chars total)"));
        let short = format!("{} ... (truncated, 500 chars total)", "x".repeat(500));
        assert!(!is_truncated(&short));
    }

    #[test]
    fn raw_text_is_written_unchanged_in_either_format() {
        let page = "<!doctype html>\n<p>a: 1</p>";
        for format in [Format::Toon, Format::Json] {
            assert_eq!(Output::Raw(page.to_owned()).encode(format), page);
        }
    }

    #[test]
    fn empty_help_is_omitted() {
        let text = Output::new(Map::new()).encode(Format::Json);
        assert_eq!(text, "{}\n");
    }
}
