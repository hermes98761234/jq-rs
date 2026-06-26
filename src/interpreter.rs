use std::collections::BTreeMap;

use crate::parser::{BinaryOp, Expr};
use crate::value::JqValue;

#[derive(Debug)]
pub struct InterpreterError {
    pub message: String,
}

impl std::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Runtime error: {}", self.message)
    }
}

impl std::error::Error for InterpreterError {}

impl InterpreterError {
    pub fn new(msg: impl Into<String>) -> Self {
        InterpreterError {
            message: msg.into(),
        }
    }
}

/// A context for variable bindings during interpretation.
#[derive(Clone)]
pub struct Context {
    pub vars: Vec<(String, JqValue)>,
}

impl Context {
    pub fn new() -> Self {
        Context { vars: Vec::new() }
    }

    pub fn push_var(&mut self, name: &str, value: JqValue) {
        self.vars.push((name.to_string(), value));
    }

    pub fn get_var(&self, name: &str) -> Option<JqValue> {
        self.vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }
}

pub struct Interpreter;

impl Interpreter {
    pub fn new() -> Self {
        Interpreter
    }

    pub fn run(&self, expr: &Expr, input: &JqValue, ctx: &mut Context) -> Result<Vec<JqValue>, InterpreterError> {
        match expr {
            Expr::Identity => Ok(vec![input.clone()]),

            Expr::FieldAccess(name) => {
                let result = input.get_field(name);
                Ok(vec![result])
            }

            Expr::IndexAccess(idx_expr) => {
                let idx_val = self.eval_to_single(idx_expr, input, ctx)?;
                let idx = match &idx_val {
                    JqValue::Number(n) => *n as i64,
                    JqValue::String(s) => {
                        // String index means field access
                        return Ok(vec![input.get_field(s)]);
                    }
                    _ => {
                        return Err(InterpreterError::new(format!(
                            "Cannot index with value of type {}",
                            idx_val.value_type()
                        )))
                    }
                };
                Ok(vec![input.get_index(idx)])
            }

            Expr::Literal(v) => Ok(vec![v.clone()]),

            Expr::Pipe(left, right) => {
                let left_results = self.run(left, input, ctx)?;
                let mut all_results = Vec::new();
                for lval in &left_results {
                    let right_results = self.run(right, lval, ctx)?;
                    all_results.extend(right_results);
                }
                Ok(all_results)
            }

            Expr::ArrayLiteral(elements) => {
                let mut arr = Vec::new();
                for elem in elements {
                    let vals = self.run(elem, input, ctx)?;
                    if vals.len() == 1 {
                        arr.push(vals[0].clone());
                    } else if vals.is_empty() {
                        // produce nothing
                    } else {
                        // multiple values: produce multiple arrays? For simplicity, take first
                        arr.push(vals[0].clone());
                    }
                }
                Ok(vec![JqValue::Array(arr)])
            }

            Expr::ObjectLiteral(pairs) => {
                let mut map = BTreeMap::new();
                for (key_expr, val_expr) in pairs {
                    let key_vals = self.run(key_expr, input, ctx)?;
                    let key = if key_vals.len() == 1 {
                        match &key_vals[0] {
                            JqValue::String(s) => s.clone(),
                            other => other.to_string(),
                        }
                    } else {
                        return Err(InterpreterError::new("Object key must be single value"));
                    };
                    let val_vals = self.run(val_expr, input, ctx)?;
                    if val_vals.len() == 1 {
                        map.insert(key, val_vals[0].clone());
                    } else if val_vals.is_empty() {
                        map.insert(key, JqValue::Null);
                    } else {
                        map.insert(key, val_vals[0].clone());
                    }
                }
                Ok(vec![JqValue::Object(map)])
            }

            Expr::IfThenElse(cond, then_branch, else_branch) => {
                let cond_val = self.eval_to_single(cond, input, ctx)?;
                if cond_val.is_truthy() {
                    self.run(then_branch, input, ctx)
                } else if let Some(else_br) = else_branch {
                    self.run(else_br, input, ctx)
                } else {
                    Ok(vec![])
                }
            }

            Expr::TryCatch(body, catch) => {
                match self.run(body, input, ctx) {
                    Ok(results) => Ok(results),
                    Err(_) => {
                        if let Some(catch_expr) = catch {
                            self.run(catch_expr, input, ctx)
                        } else {
                            Ok(vec![])
                        }
                    }
                }
            }

            Expr::FunctionCall(name, args) => {
                self.call_function(name, args, input, ctx)
            }

            Expr::Variable(name) => {
                if let Some(val) = ctx.get_var(name) {
                    Ok(vec![val])
                } else {
                    Err(InterpreterError::new(format!("Undefined variable: ${}", name)))
                }
            }

            Expr::Iterate => Ok(input.iterate()),

            Expr::Select(expr) => {
                let results: Vec<JqValue> = input
                    .iterate()
                    .into_iter()
                    .filter(|item| {
                        self.eval_to_single(expr, item, ctx)
                            .map(|v| v.is_truthy())
                            .unwrap_or(false)
                    })
                    .collect();
                Ok(results)
            }

            Expr::Map(expr) => {
                let results: Vec<JqValue> = input
                    .iterate()
                    .into_iter()
                    .flat_map(|item| self.run(expr, &item, ctx).unwrap_or_default())
                    .collect();
                Ok(results)
            }

            Expr::Reduce(expr, var, init, update) => {
                let init_val = self.eval_to_single(init, input, ctx)?;
                let items = self.run(expr, input, ctx)?;
                let mut acc = init_val;
                for item in items {
                    ctx.push_var(var, item);
                    // Evaluate update with the current accumulator as input
                    let results = self.run(update, &acc, ctx)?;
                    acc = if results.len() == 1 {
                        results[0].clone()
                    } else if results.is_empty() {
                        JqValue::Null
                    } else {
                        results[0].clone()
                    };
                    // pop var
                    if let Some(pos) = ctx.vars.iter().rposition(|(n, _)| n == var) {
                        ctx.vars.remove(pos);
                    }
                }
                Ok(vec![acc])
            }

            Expr::GroupBy(expr) => {
                let mut groups: BTreeMap<String, Vec<JqValue>> = BTreeMap::new();
                for item in input.iterate() {
                    let key_val = self.eval_to_single(expr, &item, ctx)?;
                    let key = value_to_sort_key(&key_val);
                    groups.entry(key).or_default().push(item);
                }
                let result: Vec<JqValue> = groups
                    .into_values()
                    .map(|vals| JqValue::Array(vals))
                    .collect();
                Ok(vec![JqValue::Array(result)])
            }

            Expr::SortBy(expr) => {
                let mut items: Vec<(String, JqValue)> = input
                    .iterate()
                    .into_iter()
                    .map(|item| {
                        let key = self.eval_to_single(expr, &item, ctx)?;
                        Ok::<(String, JqValue), InterpreterError>((value_to_sort_key(&key), item))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                items.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(vec![JqValue::Array(
                    items.into_iter().map(|(_, v)| v).collect(),
                )])
            }

            Expr::MinBy(expr) => {
                let items = input.iterate();
                if items.is_empty() {
                    return Ok(vec![JqValue::Null]);
                }
                let mut best: Option<(String, JqValue)> = None;
                for item in items {
                    let key = self.eval_to_single(expr, &item, ctx)?;
                    let sk = value_to_sort_key(&key);
                    if best.is_none() || sk < best.as_ref().unwrap().0 {
                        best = Some((sk, item));
                    }
                }
                Ok(vec![best.unwrap().1])
            }

            Expr::MaxBy(expr) => {
                let items = input.iterate();
                if items.is_empty() {
                    return Ok(vec![JqValue::Null]);
                }
                let mut best: Option<(String, JqValue)> = None;
                for item in items {
                    let key = self.eval_to_single(expr, &item, ctx)?;
                    let sk = value_to_sort_key(&key);
                    if best.is_none() || sk > best.as_ref().unwrap().0 {
                        best = Some((sk, item));
                    }
                }
                Ok(vec![best.unwrap().1])
            }

            Expr::UnaryMinus(inner) => {
                let val = self.eval_to_single(inner, input, ctx)?;
                match val {
                    JqValue::Number(n) => Ok(vec![JqValue::Number(-n)]),
                    _ => Err(InterpreterError::new(format!(
                        "Cannot negate value of type {}",
                        val.value_type()
                    ))),
                }
            }

            Expr::BinaryOp(op, left, right) => {
                let l = self.eval_to_single(left, input, ctx)?;
                let r = self.eval_to_single(right, input, ctx)?;
                let result = self.eval_binary_op(op, &l, &r)?;
                Ok(vec![result])
            }
        }
    }

    fn eval_to_single(&self, expr: &Expr, input: &JqValue, ctx: &mut Context) -> Result<JqValue, InterpreterError> {
        let results = self.run(expr, input, ctx)?;
        if results.len() == 1 {
            Ok(results[0].clone())
        } else if results.is_empty() {
            Ok(JqValue::Null)
        } else {
            // For conditions, multiple values are truthy if any is truthy
            // Return the first for single-value contexts
            Ok(results[0].clone())
        }
    }

    fn eval_binary_op(&self, op: &BinaryOp, left: &JqValue, right: &JqValue) -> Result<JqValue, InterpreterError> {
        match op {
            BinaryOp::Add => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Number(a + b)),
                (JqValue::String(a), JqValue::String(b)) => Ok(JqValue::String(format!("{}{}", a, b))),
                (JqValue::Array(a), JqValue::Array(b)) => {
                    let mut result = a.clone();
                    result.extend(b.clone());
                    Ok(JqValue::Array(result))
                }
                (JqValue::Object(a), JqValue::Object(b)) => {
                    let mut result = a.clone();
                    for (k, v) in b {
                        result.insert(k.clone(), v.clone());
                    }
                    Ok(JqValue::Object(result))
                }
                _ => Err(InterpreterError::new(format!(
                    "Cannot add {} and {}",
                    left.value_type(),
                    right.value_type()
                ))),
            },
            BinaryOp::Sub => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Number(a - b)),
                (JqValue::Array(a), JqValue::Array(b)) => {
                    let b_set: Vec<&JqValue> = b.iter().collect();
                    let result: Vec<JqValue> = a.iter().filter(|x| !b_set.contains(x)).cloned().collect();
                    Ok(JqValue::Array(result))
                }
                _ => Err(InterpreterError::new(format!(
                    "Cannot subtract {} and {}",
                    left.value_type(),
                    right.value_type()
                ))),
            },
            BinaryOp::Mul => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Number(a * b)),
                (JqValue::String(s), JqValue::Number(n)) => {
                    let count = *n as usize;
                    Ok(JqValue::String(s.repeat(count)))
                }
                (JqValue::Object(o), JqValue::Object(o2)) => {
                    // Merge objects, right takes precedence
                    let mut result = o.clone();
                    for (k, v) in o2 {
                        result.insert(k.clone(), v.clone());
                    }
                    Ok(JqValue::Object(result))
                }
                _ => Err(InterpreterError::new(format!(
                    "Cannot multiply {} and {}",
                    left.value_type(),
                    right.value_type()
                ))),
            },
            BinaryOp::Div => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => {
                    if *b == 0.0 {
                        Err(InterpreterError::new("Division by zero"))
                    } else {
                        Ok(JqValue::Number(a / b))
                    }
                }
                _ => Err(InterpreterError::new(format!(
                    "Cannot divide {} and {}",
                    left.value_type(),
                    right.value_type()
                ))),
            },
            BinaryOp::Mod => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => {
                    if *b == 0.0 {
                        Err(InterpreterError::new("Modulo by zero"))
                    } else {
                        Ok(JqValue::Number(a % b))
                    }
                }
                _ => Err(InterpreterError::new(format!(
                    "Cannot modulo {} and {}",
                    left.value_type(),
                    right.value_type()
                ))),
            },
            BinaryOp::Eq => Ok(JqValue::Bool(left == right)),
            BinaryOp::Neq => Ok(JqValue::Bool(left != right)),
            BinaryOp::Lt => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Bool(a < b)),
                (JqValue::String(a), JqValue::String(b)) => Ok(JqValue::Bool(a < b)),
                _ => Err(InterpreterError::new("Cannot compare")),
            },
            BinaryOp::Lte => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Bool(a <= b)),
                (JqValue::String(a), JqValue::String(b)) => Ok(JqValue::Bool(a <= b)),
                _ => Err(InterpreterError::new("Cannot compare")),
            },
            BinaryOp::Gt => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Bool(a > b)),
                (JqValue::String(a), JqValue::String(b)) => Ok(JqValue::Bool(a > b)),
                _ => Err(InterpreterError::new("Cannot compare")),
            },
            BinaryOp::Gte => match (left, right) {
                (JqValue::Number(a), JqValue::Number(b)) => Ok(JqValue::Bool(a >= b)),
                (JqValue::String(a), JqValue::String(b)) => Ok(JqValue::Bool(a >= b)),
                _ => Err(InterpreterError::new("Cannot compare")),
            },
            BinaryOp::And => {
                if left.is_truthy() && right.is_truthy() {
                    Ok(JqValue::Bool(true))
                } else {
                    Ok(JqValue::Bool(false))
                }
            }
            BinaryOp::Or => {
                if left.is_truthy() || right.is_truthy() {
                    Ok(JqValue::Bool(true))
                } else {
                    Ok(JqValue::Bool(false))
                }
            }
        }
    }

    fn call_function(&self, name: &str, args: &[Expr], input: &JqValue, ctx: &mut Context) -> Result<Vec<JqValue>, InterpreterError> {
        match name {
            "length" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("length takes no arguments"));
                }
                Ok(vec![JqValue::Number(input.length() as f64)])
            }
            "keys" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("keys takes no arguments"));
                }
                Ok(vec![input.keys()])
            }
            "type" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("type takes no arguments"));
                }
                Ok(vec![JqValue::String(input.value_type().to_string())])
            }
            "has" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("has takes 1 argument"));
                }
                let key = self.eval_to_single(&args[0], input, ctx)?;
                let key_str = match key {
                    JqValue::String(s) => s,
                    _ => key.to_string(),
                };
                Ok(vec![JqValue::Bool(input.has(&key_str))])
            }
            "in" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("in takes 1 argument"));
                }
                let needle = self.eval_to_single(&args[0], input, ctx)?;
                Ok(vec![JqValue::Bool(needle.contains(input))])
            }
            "contains" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("contains takes 1 argument"));
                }
                let needle = self.eval_to_single(&args[0], input, ctx)?;
                Ok(vec![JqValue::Bool(input.contains(&needle))])
            }
            "sort" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("sort takes no arguments"));
                }
                let mut items = input.iterate();
                items.sort_by(|a, b| value_to_sort_key(a).cmp(&value_to_sort_key(b)));
                Ok(vec![JqValue::Array(items)])
            }
            "unique" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("unique takes no arguments"));
                }
                let items = input.iterate();
                let mut seen = Vec::new();
                let mut result = Vec::new();
                for item in items {
                    if !seen.contains(&item) {
                        seen.push(item.clone());
                        result.push(item);
                    }
                }
                Ok(vec![JqValue::Array(result)])
            }
            "min" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("min takes no arguments"));
                }
                let items = input.iterate();
                if items.is_empty() {
                    return Ok(vec![JqValue::Null]);
                }
                let min = items
                    .into_iter()
                    .min_by(|a, b| value_to_sort_key(a).cmp(&value_to_sort_key(b)))
                    .unwrap();
                Ok(vec![min])
            }
            "max" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("max takes no arguments"));
                }
                let items = input.iterate();
                if items.is_empty() {
                    return Ok(vec![JqValue::Null]);
                }
                let max = items
                    .into_iter()
                    .max_by(|a, b| value_to_sort_key(a).cmp(&value_to_sort_key(b)))
                    .unwrap();
                Ok(vec![max])
            }
            "add" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("add takes no arguments"));
                }
                let items = input.iterate();
                let mut sum = 0.0f64;
                for item in items {
                    if let JqValue::Number(n) = item {
                        sum += n;
                    }
                }
                Ok(vec![JqValue::Number(sum)])
            }
            "tonumber" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("tonumber takes no arguments"));
                }
                match input {
                    JqValue::Number(_) => Ok(vec![input.clone()]),
                    JqValue::String(s) => {
                        if let Ok(n) = s.parse::<f64>() {
                            Ok(vec![JqValue::Number(n)])
                        } else {
                            Ok(vec![JqValue::Null])
                        }
                    }
                    _ => Ok(vec![JqValue::Null]),
                }
            }
            "tostring" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("tostring takes no arguments"));
                }
                Ok(vec![JqValue::String(input.to_string())])
            }
            "startswith" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("startswith takes 1 argument"));
                }
                let prefix = self.eval_to_single(&args[0], input, ctx)?;
                match (input, prefix) {
                    (JqValue::String(s), JqValue::String(p)) => Ok(vec![JqValue::Bool(s.starts_with(&*p))]),
                    _ => Ok(vec![JqValue::Bool(false)]),
                }
            }
            "endswith" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("endswith takes 1 argument"));
                }
                let suffix = self.eval_to_single(&args[0], input, ctx)?;
                match (input, suffix) {
                    (JqValue::String(s), JqValue::String(suf)) => Ok(vec![JqValue::Bool(s.ends_with(&*suf))]),
                    _ => Ok(vec![JqValue::Bool(false)]),
                }
            }
            "split" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("split takes 1 argument"));
                }
                let sep = self.eval_to_single(&args[0], input, ctx)?;
                match (input, sep) {
                    (JqValue::String(s), JqValue::String(sep)) => {
                        let parts: Vec<JqValue> = s.split(&*sep).map(|p| JqValue::String(p.to_string())).collect();
                        Ok(vec![JqValue::Array(parts)])
                    }
                    _ => Err(InterpreterError::new("split requires string arguments")),
                }
            }
            "join" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("join takes 1 argument"));
                }
                let sep = self.eval_to_single(&args[0], input, ctx)?;
                let sep_str = match sep {
                    JqValue::String(s) => s,
                    _ => return Err(InterpreterError::new("join requires string separator")),
                };
                let items = input.iterate();
                let parts: Vec<String> = items.iter().map(|v| match v {
                    JqValue::String(s) => s.clone(),
                    other => other.to_string(),
                }).collect();
                Ok(vec![JqValue::String(parts.join(&sep_str))])
            }
            "flatten" => {
                let depth = if args.len() >= 1 {
                    self.eval_to_single(&args[0], input, ctx)?
                        .length() as i32
                } else {
                    i32::MAX
                };
                let result = flatten_value(input, depth);
                Ok(vec![JqValue::Array(result)])
            }
            "reverse" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("reverse takes no arguments"));
                }
                let mut items = input.iterate();
                items.reverse();
                Ok(vec![JqValue::Array(items)])
            }
            "first" => {
                let items = input.iterate();
                Ok(if items.is_empty() {
                    vec![JqValue::Null]
                } else {
                    vec![items[0].clone()]
                })
            }
            "last" => {
                let items = input.iterate();
                Ok(if items.is_empty() {
                    vec![JqValue::Null]
                } else {
                    vec![items.last().unwrap().clone()]
                })
            }
            "nth" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("nth takes 1 argument"));
                }
                let n = self.eval_to_single(&args[0], input, ctx)?;
                let items = input.iterate();
                let idx = n.length() as usize;
                Ok(if idx < items.len() {
                    vec![items[idx].clone()]
                } else {
                    vec![JqValue::Null]
                })
            }
            "to_entries" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("to_entries takes no arguments"));
                }
                let entries: Vec<JqValue> = match input {
                    JqValue::Object(o) => o
                        .iter()
                        .map(|(k, v)| {
                            JqValue::Object(
                                vec![
                                    ("key".to_string(), JqValue::String(k.clone())),
                                    ("value".to_string(), v.clone()),
                                ]
                                .into_iter()
                                .collect(),
                            )
                        })
                        .collect(),
                    JqValue::Array(arr) => arr
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            JqValue::Object(
                                vec![
                                    ("key".to_string(), JqValue::Number(i as f64)),
                                    ("value".to_string(), v.clone()),
                                ]
                                .into_iter()
                                .collect(),
                            )
                        })
                        .collect(),
                    _ => vec![],
                };
                Ok(vec![JqValue::Array(entries)])
            }
            "from_entries" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("from_entries takes no arguments"));
                }
                let items = input.iterate();
                let mut map = BTreeMap::new();
                for item in items {
                    let key = item.get_field("key");
                    let value = item.get_field("value");
                    if let JqValue::String(k) = key {
                        map.insert(k, value);
                    }
                }
                Ok(vec![JqValue::Object(map)])
            }
            "getpath" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("getpath takes 1 argument"));
                }
                let path = self.eval_to_single(&args[0], input, ctx)?;
                let path_arr = match path {
                    JqValue::Array(arr) => arr,
                    _ => return Err(InterpreterError::new("getpath requires array argument")),
                };
                let mut current = input.clone();
                for key in path_arr {
                    current = match key {
                        JqValue::String(s) => current.get_field(&s),
                        JqValue::Number(n) => current.get_index(n as i64),
                        _ => JqValue::Null,
                    };
                }
                Ok(vec![current])
            }
            "setpath" => {
                if args.len() != 2 {
                    return Err(InterpreterError::new("setpath takes 2 arguments"));
                }
                let path = self.eval_to_single(&args[0], input, ctx)?;
                let value = self.eval_to_single(&args[1], input, ctx)?;
                let path_arr = match path {
                    JqValue::Array(arr) => arr,
                    _ => return Err(InterpreterError::new("setpath requires array argument")),
                };
                let result = set_path(input.clone(), &path_arr, &value);
                Ok(vec![result])
            }
            "delpaths" => {
                if args.len() != 1 {
                    return Err(InterpreterError::new("delpaths takes 1 argument"));
                }
                let paths = self.eval_to_single(&args[0], input, ctx)?;
                let paths_arr = match paths {
                    JqValue::Array(arr) => arr,
                    _ => return Err(InterpreterError::new("delpaths requires array argument")),
                };
                let mut result = input.clone();
                for path in paths_arr {
                    let path_arr = match path {
                        JqValue::Array(arr) => arr,
                        _ => continue,
                    };
                    result = del_path(result, &path_arr);
                }
                Ok(vec![result])
            }
            "all" => {
                if args.is_empty() {
                    // all values are truthy
                    let all_true = input.iterate().iter().all(|v| v.is_truthy());
                    return Ok(vec![JqValue::Bool(all_true)]);
                }
                let cond = &args[0];
                let all_true = input.iterate().iter().all(|item| {
                    self.eval_to_single(cond, &item, ctx)
                        .map(|v| v.is_truthy())
                        .unwrap_or(false)
                });
                Ok(vec![JqValue::Bool(all_true)])
            }
            "any" => {
                if args.is_empty() {
                    let any_true = input.iterate().iter().any(|v| v.is_truthy());
                    return Ok(vec![JqValue::Bool(any_true)]);
                }
                let cond = &args[0];
                let any_true = input.iterate().iter().any(|item| {
                    self.eval_to_single(cond, &item, ctx)
                        .map(|v| v.is_truthy())
                        .unwrap_or(false)
                });
                Ok(vec![JqValue::Bool(any_true)])
            }
            "error" => {
                let msg = if args.is_empty() {
                    "".to_string()
                } else {
                    self.eval_to_single(&args[0], input, ctx)?.to_string()
                };
                Err(InterpreterError::new(msg))
            }
            _ => Err(InterpreterError::new(format!("Unknown function: {}", name))),
        }
    }
}

fn value_to_sort_key(v: &JqValue) -> String {
    match v {
        JqValue::Null => "\0".to_string(),
        JqValue::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        JqValue::Number(n) => format!("{:+e}", n),
        JqValue::String(s) => format!("s{}", s),
        JqValue::Array(_) => format!("a{}", v.to_string()),
        JqValue::Object(_) => format!("o{}", v.to_string()),
    }
}

fn flatten_value(v: &JqValue, depth: i32) -> Vec<JqValue> {
    if depth <= 0 {
        return vec![v.clone()];
    }
    match v {
        JqValue::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.extend(flatten_value(item, depth - 1));
            }
            result
        }
        _ => vec![v.clone()],
    }
}

fn set_path(mut root: JqValue, path: &[JqValue], value: &JqValue) -> JqValue {
    if path.is_empty() {
        return value.clone();
    }
    let key = &path[0];
    let rest = &path[1..];
    match key {
        JqValue::String(s) => {
            if let JqValue::Object(map) = &mut root {
                let mut child = map.get(s).cloned().unwrap_or(JqValue::Null);
                child = set_path(child, rest, value);
                map.insert(s.clone(), child);
            }
        }
        JqValue::Number(n) => {
            if let JqValue::Array(arr) = &mut root {
                let idx = *n as i64;
                let real_idx = if idx < 0 {
                    (arr.len() as i64 + idx) as usize
                } else {
                    idx as usize
                };
                if real_idx < arr.len() {
                    arr[real_idx] = set_path(arr[real_idx].clone(), rest, value);
                }
            }
        }
        _ => {}
    }
    root
}

fn del_path(mut root: JqValue, path: &[JqValue]) -> JqValue {
    if path.is_empty() {
        return root;
    }
    if path.len() == 1 {
        let key = &path[0];
        match key {
            JqValue::String(s) => {
                if let JqValue::Object(map) = &mut root {
                    map.remove(s);
                }
            }
            JqValue::Number(n) => {
                if let JqValue::Array(arr) = &mut root {
                    let idx = *n as i64;
                    let real_idx = if idx < 0 {
                        (arr.len() as i64 + idx) as usize
                    } else {
                        idx as usize
                    };
                    if real_idx < arr.len() {
                        arr.remove(real_idx);
                    }
                }
            }
            _ => {}
        }
        return root;
    }
    let key = &path[0];
    let rest = &path[1..];
    match key {
        JqValue::String(s) => {
            if let JqValue::Object(map) = &mut root {
                if let Some(child) = map.get(s) {
                    let new_child = del_path(child.clone(), rest);
                    map.insert(s.clone(), new_child);
                }
            }
        }
        JqValue::Number(n) => {
            if let JqValue::Array(arr) = &mut root {
                let idx = *n as i64;
                let real_idx = if idx < 0 {
                    (arr.len() as i64 + idx) as usize
                } else {
                    idx as usize
                };
                if real_idx < arr.len() {
                    arr[real_idx] = del_path(arr[real_idx].clone(), rest);
                }
            }
        }
        _ => {}
    }
    root
}
