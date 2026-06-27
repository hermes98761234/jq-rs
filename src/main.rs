mod interpreter;
mod parser;
mod value;

use anyhow::{anyhow, Context as AnyhowContext};
use clap::{Arg, Command};
use std::io::{self, Read, Write};

use interpreter::{Context, Interpreter};
use parser::Parser;
use value::JqValue;

fn main() -> anyhow::Result<()> {
    let matches = Command::new("jq-rs")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A Rust reimplementation of jq — the JSON processor")
        .arg(
            Arg::new("compact")
                .short('c')
                .long("compact-output")
                .action(clap::ArgAction::SetTrue)
                .help("Compact output (no pretty-print)"),
        )
        .arg(
            Arg::new("raw")
                .short('r')
                .long("raw-output")
                .action(clap::ArgAction::SetTrue)
                .help("Output raw strings rather than JSON-encoded"),
        )
        .arg(
            Arg::new("slurp")
                .short('s')
                .long("slurp")
                .action(clap::ArgAction::SetTrue)
                .help("Slurp all inputs into an array"),
        )
        .arg(
            Arg::new("null-input")
                .short('n')
                .long("null-input")
                .action(clap::ArgAction::SetTrue)
                .help("Use null as input (for generating JSON)"),
        )
        .arg(
            Arg::new("program-file")
                .short('f')
                .long("from-file")
                .help("Read filter from a file instead of command line"),
        )
        .arg(
            Arg::new("monochrome")
                .short('M')
                .long("monochrome-output")
                .action(clap::ArgAction::SetTrue)
                .help("Disable colored output"),
        )
        .arg(
            Arg::new("filter")
                .help("The jq filter expression")
                .required(false),
        )
        .arg(
            Arg::new("files")
                .help("Input JSON files (reads stdin if none given)")
                .num_args(0..)
                .value_name("FILE"),
        )
        .arg(
            Arg::new("args")
                .help("Positional arguments to pass to the filter")
                .num_args(0..)
                .last(true),
        )
        .get_matches();

    let compact = matches.get_flag("compact");
    let raw_output = matches.get_flag("raw");
    let slurp = matches.get_flag("slurp");
    let null_input = matches.get_flag("null-input");
    let monochrome = matches.get_flag("monochrome");
    use std::io::IsTerminal;
    let colored = std::io::stdout().is_terminal() && !monochrome;

    // Parse the filter expression
    let filter_str = if let Some(filter) = matches.get_one::<String>("filter") {
        if filter.starts_with('-') {
            return Err(anyhow!(
                "Filter expression cannot start with '-'. Use -- before the filter if needed."
            ));
        }
        filter.clone()
    } else if let Some(path) = matches.get_one::<String>("program-file") {
        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read filter file: {}", path))?
    } else {
        return Err(anyhow!("No filter expression provided"));
    };

    let mut parser = Parser::new(&filter_str);
    let expr = parser.parse().map_err(|e| anyhow!("Parse error: {}", e))?;

    // Always collect stdin/file values (needed for `inputs` builtin even in -n mode)
    let file_paths: Vec<String> = matches
        .get_many::<String>("files")
        .unwrap_or_default()
        .cloned()
        .collect();

    let mut raw_inputs: Vec<(Option<String>, JqValue)> = Vec::new();
    if file_paths.is_empty() {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .with_context(|| "Failed to read from stdin")?;
        for v in parse_json_stream(text.trim()) {
            raw_inputs.push((None, v));
        }
    } else {
        for path in &file_paths {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path))?;
            for v in parse_json_stream(text.trim()) {
                raw_inputs.push((Some(path.clone()), v));
            }
        }
    }
    let all_raw_values: Vec<JqValue> = raw_inputs.iter().map(|(_, v)| v.clone()).collect();

    // Build all_inputs: what gets processed as main inputs
    let all_inputs: Vec<(Option<String>, JqValue)> = if null_input {
        vec![(None, JqValue::Null)]
    } else if slurp {
        let values: Vec<JqValue> = raw_inputs.iter().map(|(_, v)| v.clone()).collect();
        vec![(None, JqValue::Array(values))]
    } else {
        raw_inputs
    };

    let interpreter = Interpreter::new();

    // Process each input
    let mut outputs: Vec<String> = Vec::new();

    for (i, (filename, input)) in all_inputs.iter().enumerate() {
        let mut ctx = Context::new();
        ctx.input_filename = filename.clone();
        // In null-input mode all stdin values are available via `inputs`;
        // otherwise, remaining inputs are the ones after the current index.
        ctx.remaining_inputs = if null_input {
            all_raw_values.clone()
        } else {
            all_inputs[i + 1..].iter().map(|(_, v)| v.clone()).collect()
        };

        let results = interpreter
            .run(&expr, input, &mut ctx)
            .map_err(|e| anyhow!("{}", e))?;
        for result in results {
            let output_str = format_output(result, compact, raw_output, colored);
            outputs.push(output_str);
        }
    }

    // Write outputs
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 && !compact {
            writeln!(stdout_lock)?;
        }
        write!(stdout_lock, "{}", output)?;
        if !raw_output && !output.ends_with('\n') {
            writeln!(stdout_lock)?;
        }
    }

    Ok(())
}

fn parse_json_stream(text: &str) -> Vec<JqValue> {
    let mut values = Vec::new();
    let trimmed = text.trim();

    // Try parsing as a standard JSON value first
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return vec![JqValue::from(v)];
    }

    // If that fails, try to parse multiple JSON values (stream)
    let mut offset = 0;
    while offset < trimmed.len() {
        let substr = &trimmed[offset..];
        let substr_trimmed = substr.trim_start();
        if substr_trimmed.is_empty() {
            break;
        }
        // Try incremental parsing using serde_json
        match try_parse_json_prefix(substr_trimmed) {
            Some(v) => {
                values.push(JqValue::from(v));
                let consumed =
                    substr.len() - substr_trimmed.len() + json_consumed_length(substr_trimmed);
                offset += consumed;
            }
            None => break,
        }
    }

    if values.is_empty() {
        // Fallback: just try the full string
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            values.push(JqValue::from(v));
        }
    }

    values
}

fn try_parse_json_prefix(s: &str) -> Option<serde_json::Value> {
    // Use a deserializer approach: try to parse and track position
    let mut deserializer = serde_json::Deserializer::from_str(s);
    let result: Result<serde_json::Value, _> = serde::Deserialize::deserialize(&mut deserializer);
    result.ok()
}

fn json_consumed_length(s: &str) -> usize {
    // Determine how many bytes of the string were consumed by the first JSON value.
    // Handles all JSON token types: objects, arrays, strings, numbers, bools, null.
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (i, c) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match c {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match c {
                '{' | '[' => depth += 1,
                '}' | ']' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                '"' => in_string = true,
                't' | 'f' | 'n' if depth == 0 => {
                    // At top level: could be true, false, or null
                    let rest = &s[i..];
                    if rest.starts_with("true") {
                        return i + 4;
                    } else if rest.starts_with("false") {
                        return i + 5;
                    } else if rest.starts_with("null") {
                        return i + 4;
                    }
                }
                c if (c.is_ascii_digit() || c == '-') && depth == 0 => {
                    // A number at top level — scan through the full number literal
                    let mut end = i + 1;
                    let bytes = s.as_bytes();
                    while end < bytes.len() {
                        let b = bytes[end];
                        if b.is_ascii_digit()
                            || b == b'.'
                            || b == b'e'
                            || b == b'E'
                            || b == b'+'
                            || b == b'-'
                        {
                            end += 1;
                        } else {
                            break;
                        }
                    }
                    return end;
                }
                _ => {}
            }
        }
    }
    // Fallback: return the full string length
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::JqValue;

    #[test]
    fn test_colorize_string_contains_ansi() {
        let val = JqValue::String("hello".to_string());
        let out = format_output(val, false, false, true);
        assert!(out.contains("\x1b["), "Expected ANSI codes in colored output");
        assert!(out.contains("hello"), "Expected string content");
    }

    #[test]
    fn test_no_color_when_disabled() {
        let val = JqValue::String("hello".to_string());
        let out = format_output(val, false, false, false);
        assert!(!out.contains("\x1b["), "Expected no ANSI codes");
    }

    #[test]
    fn test_colorize_number() {
        let val = JqValue::Number(42.0);
        let out = format_output(val, false, false, true);
        assert!(out.contains("42"), "Expected number in output");
    }
}

fn format_output(value: JqValue, compact: bool, raw_output: bool, colored: bool) -> String {
    if raw_output {
        return match &value {
            JqValue::String(s) => s.clone(),
            JqValue::Null => "null".to_string(),
            _ => colorize_json(&value, 0, false),
        };
    }
    if compact {
        let sval: serde_json::Value = value.into();
        return serde_json::to_string(&sval).unwrap_or_default();
    }
    colorize_json(&value, 0, colored)
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[0;32m";
const BLUE: &str = "\x1b[0;34m";
const DARK: &str = "\x1b[1;30m";

fn colorize_json(val: &JqValue, indent: usize, colored: bool) -> String {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    match val {
        JqValue::Null => {
            if colored {
                format!("{BOLD}null{RESET}")
            } else {
                "null".to_string()
            }
        }
        JqValue::Bool(b) => {
            let s = if *b { "true" } else { "false" };
            if colored {
                format!("{BOLD}{s}{RESET}")
            } else {
                s.to_string()
            }
        }
        JqValue::Number(n) => {
            let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            };
            if colored {
                format!("{BLUE}{s}{RESET}")
            } else {
                s
            }
        }
        JqValue::String(s) => {
            let escaped = serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s));
            if colored {
                format!("{GREEN}{escaped}{RESET}")
            } else {
                escaped
            }
        }
        JqValue::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            let mut out = "[\n".to_string();
            for (i, item) in arr.iter().enumerate() {
                out.push_str(&pad1);
                out.push_str(&colorize_json(item, indent + 1, colored));
                if i + 1 < arr.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push(']');
            out
        }
        JqValue::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut out = "{\n".to_string();
            let entries: Vec<_> = map.iter().collect();
            for (i, (k, v)) in entries.iter().enumerate() {
                let key_str = serde_json::to_string(k).unwrap_or_else(|_| format!("\"{}\"", k));
                out.push_str(&pad1);
                if colored {
                    out.push_str(&format!("{BOLD}{BLUE}{key_str}{RESET}"));
                } else {
                    out.push_str(&key_str);
                }
                out.push_str(": ");
                out.push_str(&colorize_json(v, indent + 1, colored));
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push('}');
            out
        }
    }
}
