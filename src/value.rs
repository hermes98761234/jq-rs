use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

/// A JSON value, matching serde_json::Value but with our own impl for flexibility.
#[derive(Debug, Clone, PartialEq)]
pub enum JqValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JqValue>),
    Object(BTreeMap<String, JqValue>),
}

impl JqValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            JqValue::Null => false,
            JqValue::Bool(b) => *b,
            JqValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JqValue::String(s) => !s.is_empty(),
            JqValue::Array(a) => !a.is_empty(),
            JqValue::Object(o) => !o.is_empty(),
        }
    }

    pub fn value_type(&self) -> &str {
        match self {
            JqValue::Null => "null",
            JqValue::Bool(_) => "boolean",
            JqValue::Number(_) => "number",
            JqValue::String(_) => "string",
            JqValue::Array(_) => "array",
            JqValue::Object(_) => "object",
        }
    }

    /// Get a field from an object, or null.
    pub fn get_field(&self, key: &str) -> JqValue {
        match self {
            JqValue::Object(map) => map.get(key).cloned().unwrap_or(JqValue::Null),
            _ => JqValue::Null,
        }
    }

    /// Get an index from an array, or null.
    pub fn get_index(&self, idx: i64) -> JqValue {
        match self {
            JqValue::Array(arr) => {
                let real_idx = if idx < 0 {
                    (arr.len() as i64 + idx) as usize
                } else {
                    idx as usize
                };
                arr.get(real_idx).cloned().unwrap_or(JqValue::Null)
            }
            _ => JqValue::Null,
        }
    }

    /// Iterate over array elements or object values.
    pub fn iterate(&self) -> Vec<JqValue> {
        match self {
            JqValue::Array(arr) => arr.clone(),
            JqValue::Object(o) => o.values().cloned().collect(),
            _ => vec![self.clone()],
        }
    }

    /// Get the length of the value.
    pub fn length(&self) -> i64 {
        match self {
            JqValue::Array(arr) => arr.len() as i64,
            JqValue::Object(o) => o.len() as i64,
            JqValue::String(s) => s.chars().count() as i64,
            _ => 0,
        }
    }

    /// Get sorted keys of an object, or sorted keys of an array of objects.
    pub fn keys(&self) -> JqValue {
        match self {
            JqValue::Object(o) => {
                let mut keys: Vec<String> = o.keys().cloned().collect();
                keys.sort();
                JqValue::Array(keys.into_iter().map(JqValue::String).collect())
            }
            JqValue::Array(arr) => {
                let mut all_keys: Vec<String> = Vec::new();
                for item in arr {
                    if let JqValue::Object(o) = item {
                        for k in o.keys() {
                            if !all_keys.contains(k) {
                                all_keys.push(k.clone());
                            }
                        }
                    }
                }
                all_keys.sort();
                JqValue::Array(all_keys.into_iter().map(JqValue::String).collect())
            }
            _ => JqValue::Array(vec![]),
        }
    }

    /// Check if the value contains another value (deep search).
    pub fn contains(&self, needle: &JqValue) -> bool {
        if self == needle {
            return true;
        }
        match self {
            JqValue::Array(arr) => arr.iter().any(|v| v.contains(needle)),
            JqValue::Object(o) => o.values().any(|v| v.contains(needle)),
            _ => false,
        }
    }

    /// Check if a key exists in an object.
    pub fn has(&self, key: &str) -> bool {
        match self {
            JqValue::Object(o) => o.contains_key(key),
            _ => false,
        }
    }
}

impl fmt::Display for JqValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JqValue::Null => write!(f, "null"),
            JqValue::Bool(b) => write!(f, "{}", b),
            JqValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            JqValue::String(s) => write!(f, "\"{}\"", s),
            JqValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            JqValue::Object(o) => {
                write!(f, "{{")?;
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl Serialize for JqValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            JqValue::Null => serializer.serialize_none(),
            JqValue::Bool(b) => serializer.serialize_bool(*b),
            JqValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
                    serializer.serialize_i64(*n as i64)
                } else {
                    serializer.serialize_f64(*n)
                }
            }
            JqValue::String(s) => serializer.serialize_str(s),
            JqValue::Array(arr) => arr.serialize(serializer),
            JqValue::Object(o) => o.serialize(serializer),
        }
    }
}

impl From<serde_json::Value> for JqValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => JqValue::Null,
            serde_json::Value::Bool(b) => JqValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    JqValue::Number(i as f64)
                } else if let Some(f) = n.as_f64() {
                    JqValue::Number(f)
                } else {
                    JqValue::Number(0.0)
                }
            }
            serde_json::Value::String(s) => JqValue::String(s),
            serde_json::Value::Array(arr) => {
                JqValue::Array(arr.into_iter().map(JqValue::from).collect())
            }
            serde_json::Value::Object(o) => {
                JqValue::Object(o.into_iter().map(|(k, v)| (k, JqValue::from(v))).collect())
            }
        }
    }
}

impl From<JqValue> for serde_json::Value {
    fn from(v: JqValue) -> Self {
        match v {
            JqValue::Null => serde_json::Value::Null,
            JqValue::Bool(b) => serde_json::Value::Bool(b),
            JqValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < (i64::MAX as f64) {
                    serde_json::Value::Number(serde_json::Number::from(n as i64))
                } else {
                    serde_json::Number::from_f64(n)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
            }
            JqValue::String(s) => serde_json::Value::String(s),
            JqValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            JqValue::Object(o) => serde_json::Value::Object(
                o.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
        }
    }
}
