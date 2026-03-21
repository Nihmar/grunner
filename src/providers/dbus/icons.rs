//! Icon parsing for D-Bus search provider results

use super::types::IconData;
use zbus::zvariant::OwnedValue;

/// Parse icon data from a D-Bus variant value
///
/// GNOME Shell search providers can send icons in several complex formats:
/// - Simple string (themed icon name)
/// - Structure with type and payload (themed-icon or file-icon)
/// - Nested variants and dictionaries
#[must_use]
pub fn parse_icon_variant(val: &OwnedValue) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn inner(v: &Value<'_>) -> Option<IconData> {
        match v {
            Value::Value(inner_v) => inner(inner_v),
            Value::Structure(s) => {
                let fields = s.fields();
                if fields.len() >= 2
                    && let Value::Str(type_name) = &fields[0]
                {
                    return match type_name.as_str() {
                        "themed-icon" => extract_themed(&fields[1]),
                        "file-icon" => extract_file(&fields[1]),
                        _ => None,
                    };
                }
                fields.iter().find_map(inner)
            }
            Value::Str(s) => {
                let s = s.as_str();
                if !s.is_empty() && !s.contains(' ') {
                    Some(IconData::Themed(s.to_string()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    inner(val)
}

fn extract_themed(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn first_name_from_array(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Array(a) => a.iter().find_map(|item| match item {
                Value::Str(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
                Value::Value(inner) => first_name_from_array(inner),
                _ => None,
            }),
            Value::Value(inner) => first_name_from_array(inner),
            Value::Str(s) if !s.as_str().is_empty() => Some(s.as_str().to_string()),
            _ => None,
        }
    }

    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Dict(d) => {
                for (k, val) in d.iter() {
                    if let (Value::Str(key), v2) = (k, val)
                        && key.as_str() == "names"
                        && let Some(name) = first_name_from_array(v2)
                    {
                        return Some(name);
                    }
                }
                for (_, val) in d.iter() {
                    if let Some(name) = first_name_from_array(val) {
                        return Some(name);
                    }
                }
                None
            }
            Value::Array(_) => first_name_from_array(v),
            _ => None,
        }
    }

    walk(val).map(IconData::Themed)
}

fn extract_file(val: &zbus::zvariant::Value<'_>) -> Option<IconData> {
    use zbus::zvariant::Value;

    fn walk(v: &Value<'_>) -> Option<String> {
        match v {
            Value::Value(inner) => walk(inner),
            Value::Str(s) => {
                let s = s.as_str();
                if s.is_empty() {
                    None
                } else {
                    let path = s.strip_prefix("file://").unwrap_or(s);
                    Some(path.to_string())
                }
            }
            Value::Dict(d) => {
                for (k, val) in d.iter() {
                    if let Value::Str(key) = k
                        && key.as_str() == "file"
                        && let Some(p) = walk(val)
                    {
                        return Some(p);
                    }
                }
                d.iter().find_map(|(_, v)| walk(v))
            }
            _ => None,
        }
    }

    walk(val).map(IconData::File)
}
