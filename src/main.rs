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
    let mut parser = Parser::new(&filter_str);
    let expr = parser.parse().map_err(|e| anyhow!("Parse error: {}", e))?;

    // Read all input
    let mut input_text = String::new();
    io::stdin()
        .read_to_string(&mut input_text)
        .with_context(|| "Failed to read from stdin")?;

    // Parse input JSON(s)
    let input_values: Vec<JqValue> = if null_input {
        vec![JqValue::Null]
    } else {
        let trimmed = input_text.trim();
        if trimmed.is_empty() {
            vec![]
        } else if slurp {
            // Try to parse as a stream and wrap in array
            let values = parse_json_stream(trimmed);
            if values.is_empty() {
                vec![]
            } else {
                vec![JqValue::Array(values)]
            }
        } else {
            parse_json_stream(trimmed)
        }
    };

    let interpreter = Interpreter::new();
    let mut ctx = Context::new();

    // Process each input
    let mut outputs: Vec<String> = Vec::new();

    for input in &input_values {
        let results = interpreter.run(&expr, input, &mut ctx).map_err(|e| anyhow!("{}", e))?;
        for result in results {
            let output_str = format_output(result, compact, raw_output);
            outputs.push(output_str);
        }
    }

    // Write outputs
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 && !compact {
            // Newline between JSON output objects
            writeln!(stdout_lock)?;
        }
        write!(stdout_lock, "{}", output)?;
        if !output.ends_with('\n') && !raw_output {
            writeln!(stdout_lock)?;
        }
    }
    if raw_output {
        for output in &outputs {
            write!(stdout_lock, "{}", output)?;
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
                let consumed = substr.len() - substr_trimmed.len() + json_consumed_length(substr_trimmed);
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
    // Estimate how many bytes of the string were consumed by the deserializer
    // Use a simple heuristic: find the end of the first JSON value
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
                _ => {}
            }
        }
    }
    // Fallback: if it's a simple value (number, string, bool, null), estimate
    s.find(|c: char| c == '\n').unwrap_or(s.len())
}

fn format_output(value: JqValue, compact: bool, raw_output: bool) -> String {
    if raw_output {
        match &value {
            JqValue::String(s) => s.clone(),
            JqValue::Null => "null".to_string(),
            _ => {
                // For raw output, format as JSON but use to_string()
                format!("{}", value)
            }
        }
    } else {
        if compact {
            // Use serde_json compact format
            let sval: serde_json::Value = value.into();
            serde_json::to_string(&sval).unwrap_or_default()
        } else {
            // Pretty print using serde_json
            let sval: serde_json::Value = value.into();
            serde_json::to_string_pretty(&sval).unwrap_or_default()
        }
    }
}
