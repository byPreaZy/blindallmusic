//! Minimal PHP `serialize()` encoder — writes cache values readable by
//! Laravel (`Cache::put` serializes the same way). Used for keys shared
//! with the PHP app (e.g. `weekly-top-10-users`).

use serde_json::{Map, Value};

/// `serialize($value)` for null/bool/int/float/string/array.
/// Objects/assoc arrays → `a:…` like PHP arrays; lists → `a:…` with int keys.
pub fn to_php(v: &Value) -> String {
    match v {
        Value::Null => "N;".into(),
        Value::Bool(b) => format!("b:{};", *b as i32),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                format!("i:{i};")
            } else if let Some(f) = n.as_f64() {
                // PHP serializes floats with `d:`.
                format!("d:{f};")
            } else {
                "i:0;".into()
            }
        }
        Value::String(s) => format!("s:{}:\"{}\";", s.len(), s),
        Value::Array(items) => {
            let mut out = format!("a:{}:{{", items.len());
            for (i, item) in items.iter().enumerate() {
                out.push_str(&format!("i:{i};"));
                out.push_str(&to_php(item));
            }
            out.push('}');
            out
        }
        Value::Object(map) => {
            let mut out = format!("a:{}:{{", map.len());
            for (k, item) in map {
                out.push_str(&to_php(&Value::String(k.clone())));
                out.push_str(&to_php(item));
            }
            out.push('}');
            out
        }
    }
}

/// `unserialize()` for the subset we read back (strings, ints, arrays).
/// Kept minimal — used only where we must parse a PHP-written value.
pub fn from_php(input: &str) -> Option<Value> {
    let mut p = Parser { s: input.as_bytes(), i: 0 };
    let v = p.value()?;
    (p.i == input.len()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn value(&mut self) -> Option<Value> {
        match *self.s.get(self.i)? {
            b'N' => {
                self.i += 2;
                Some(Value::Null)
            }
            b'b' => {
                self.i += 2;
                let n = self.until(b';')?;
                Some(Value::Bool(n == "1"))
            }
            b'i' | b'd' => {
                self.i += 2;
                let n = self.until(b';')?;
                if let Ok(i) = n.parse::<i64>() {
                    Some(i.into())
                } else {
                    n.parse::<f64>().ok().map(|f| f.into())
                }
            }
            b's' => {
                self.i += 2;
                let _len: usize = self.until(b':')?.parse().ok()?;
                self.i += 1; // opening quote
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i] != b'"' {
                    self.i += 1;
                }
                let raw = std::str::from_utf8(&self.s[start..self.i]).ok()?;
                self.i += 2; // closing quote + ;
                Some(Value::String(raw.to_string()))
            }
            b'a' => {
                self.i += 2;
                let _n: usize = self.until(b':')?.parse().ok()?;
                self.i += 1; // {
                let mut map = Map::new();
                loop {
                    if self.i >= self.s.len() {
                        return None;
                    }
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    let key = self.value()?;
                    let val = self.value()?;
                    match key {
                        Value::Number(n) => {
                            map.insert(n.to_string(), val);
                        }
                        Value::String(s) => {
                            map.insert(s, val);
                        }
                        _ => return None,
                    }
                }
                Some(Value::Object(map))
            }
            _ => None,
        }
    }

    fn until(&mut self, b: u8) -> Option<String> {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i] != b {
            self.i += 1;
        }
        let out = std::str::from_utf8(&self.s[start..self.i]).ok()?.to_string();
        self.i += 1;
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scalars() {
        assert_eq!(to_php(&json!(null)), "N;");
        assert_eq!(to_php(&json!(true)), "b:1;");
        assert_eq!(to_php(&json!(42)), "i:42;");
        assert_eq!(to_php(&json!("hi")), "s:2:\"hi\";");
    }

    #[test]
    fn list_and_assoc() {
        assert_eq!(to_php(&json!([1, "a"])), "a:2:{i:0;i:1;i:1;s:1:\"a\";}");
        assert_eq!(
            to_php(&json!({"user_id": 7})),
            "a:1:{s:7:\"user_id\";i:7;}"
        );
    }

    #[test]
    fn string_roundtrip() {
        let v = from_php("s:5:\"hello\";").unwrap();
        assert_eq!(v, json!("hello"));
    }
}
