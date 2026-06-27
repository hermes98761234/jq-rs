mod interpreter;
mod parser;
mod value;

use anyhow::{anyhow, Context as AnyhowContext};
use clap::{Arg, Command};
use std::io::{self, Write};

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
        .arg(
            Arg::new("stream")
                .long("stream")
                .action(clap::ArgAction::SetTrue)
                .help("Output streaming path/value events instead of filter results"),
        )
        .get_matches();

    let compact = matches.get_flag("compact");
    let raw_output = matches.get_flag("raw");
    let slurp = matches.get_flag("slurp");
    let null_input = matches.get_flag("null-input");
    let monochrome = matches.get_flag("monochrome");
    use std::io::IsTerminal;
    let colored = std::io::stdout().is_terminal() && !monochrome;
    let stream_mode = matches.get_flag("stream");

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
        let reader = io::BufReader::new(io::stdin());
        let vals = parse_json_from_reader(reader);
        for v in vals {
            raw_inputs.push((None, v));
        }
    } else {
        for path in &file_paths {
            let file = std::fs::File::open(path)
                .with_context(|| format!("Failed to open file: {}", path))?;
            let reader = io::BufReader::new(file);
            let vals = parse_json_from_reader(reader);
            for v in vals {
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

        if stream_mode {
            let events = stream_value(&[], input);
            for event in events {
                let output_str = format_output(event, compact, raw_output, false);
                outputs.push(output_str);
            }
        } else {
            let results = interpreter
                .run(&expr, input, &mut ctx)
                .map_err(|e| anyhow!("{}", e))?;
            for result in results {
                let output_str = format_output(result, compact, raw_output, colored);
                outputs.push(output_str);
            }
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

fn parse_json_from_reader<R: std::io::Read>(reader: R) -> Vec<JqValue> {
    serde_json::Deserializer::from_reader(reader)
        .into_iter::<serde_json::Value>()
        .filter_map(|r| r.ok())
        .map(JqValue::from)
        .collect()
}

pub fn stream_value(path: &[JqValue], val: &JqValue) -> Vec<JqValue> {
    let mut events = Vec::new();
    match val {
        JqValue::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(JqValue::Number(i as f64));
                events.extend(stream_value(&child_path, item));
            }
            // truncated marker
            let mut trunc = std::collections::BTreeMap::new();
            trunc.insert("truncated".to_string(), JqValue::Bool(true));
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                JqValue::Object(trunc),
            ]));
        }
        JqValue::Object(map) => {
            for (k, v) in map {
                let mut child_path = path.to_vec();
                child_path.push(JqValue::String(k.clone()));
                events.extend(stream_value(&child_path, v));
            }
            let mut trunc = std::collections::BTreeMap::new();
            trunc.insert("truncated".to_string(), JqValue::Bool(true));
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                JqValue::Object(trunc),
            ]));
        }
        _ => {
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                val.clone(),
            ]));
        }
    }
    events
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

    #[test]
    fn test_stream_value_scalar() {
        let val = JqValue::Number(42.0);
        let events = stream_value(&[], &val);
        assert_eq!(events.len(), 1);
        // [[], 42]
        if let JqValue::Array(pair) = &events[0] {
            assert_eq!(&pair[0], &JqValue::Array(vec![]));
            assert_eq!(&pair[1], &JqValue::Number(42.0));
        } else {
            panic!("Expected array pair");
        }
    }

    #[test]
    fn test_stream_value_object() {
        let mut map = std::collections::BTreeMap::new();
        map.insert("a".to_string(), JqValue::Number(1.0));
        let val = JqValue::Object(map);
        let events = stream_value(&[], &val);
        // [[["a"],1], [["a"],{"truncated":true}]]
        assert!(events.len() >= 2, "Expected at least 2 events for object");
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
