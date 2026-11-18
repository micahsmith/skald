use serde_json::{Map, Value, json};

use crate::output::Output;

const USAGE_CODES: [&str; 9] = [
    "unknown-flag",
    "unknown-command",
    "unexpected-argument",
    "missing-argument",
    "missing-command",
    "invalid-value",
    "invalid-kind",
    "unknown-field",
    "invalid-usage",
];

#[derive(Debug)]
pub struct SkaldError {
    pub code: &'static str,
    pub message: String,
    pub problems: Vec<Value>,
    pub help: Vec<String>,
    pub exit_code: u8,
}

impl SkaldError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        let exit_code = if USAGE_CODES.contains(&code) { 2 } else { 1 };
        Self {
            code,
            message: message.into(),
            problems: Vec::new(),
            help: Vec::new(),
            exit_code,
        }
    }

    pub fn with_help(mut self, line: impl Into<String>) -> Self {
        self.help.push(line.into());
        self
    }

    pub fn into_output(self) -> Output {
        let mut value = Map::new();
        value.insert(
            "error".to_owned(),
            json!({"code": self.code, "message": self.message}),
        );
        if !self.problems.is_empty() {
            value.insert("problems".to_owned(), Value::Array(self.problems));
        }
        Output::new(value).with_help_lines(self.help)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Format;

    #[test]
    fn usage_codes_exit_2_and_others_exit_1() {
        assert_eq!(SkaldError::new("unknown-flag", "").exit_code, 2);
        assert_eq!(SkaldError::new("unknown-command", "").exit_code, 2);
        assert_eq!(SkaldError::new("missing-argument", "").exit_code, 2);
        assert_eq!(SkaldError::new("not-found", "").exit_code, 1);
    }

    #[test]
    fn error_object_has_code_message_and_help() {
        let text = SkaldError::new("unknown-flag", "unknown flag --bogus")
            .with_help("Valid flags: --json, --help")
            .into_output()
            .encode(Format::Json);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            json!({
                "error": {"code": "unknown-flag", "message": "unknown flag --bogus"},
                "help": ["Valid flags: --json, --help"]
            })
        );
    }

    #[test]
    fn problems_follow_the_error() {
        let mut error = SkaldError::new("invalid-record", "the record has 1 problem");
        error
            .problems
            .push(json!({"path": "/entities/0", "code": "missing-name"}));
        let text = error.into_output().encode(Format::Json);
        let value: Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["error", "problems"]);
    }
}
