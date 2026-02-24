use evalexpr::*;

/// Controlla se la query contiene solo caratteri ammessi in un'espressione aritmetica.
pub fn is_arithmetic_query(s: &str) -> bool {
    s.chars().all(|c| {
        c.is_ascii_digit()
            || c == '.'
            || c == '+'
            || c == '-'
            || c == '*'
            || c == '/'
            || c == '%'
            || c == '^'
            || c == '('
            || c == ')'
            || c.is_whitespace()
    })
}

/// Converte i numeri interi in float aggiungendo ".0".
/// Esempio: "7/5" → "7.0/5.0", così la divisione produce un risultato decimale.
fn ensure_float_literals(expr: &str) -> String {
    let mut result = String::new();
    let mut chars = expr.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            // Inizio di un numero
            let mut num = c.to_string();
            // Raccogli le cifre successive
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() {
                    num.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            // Controlla se il prossimo carattere è un punto (numero già float)
            if let Some(&next) = chars.peek() {
                if next == '.' {
                    // È già un float, consuma il punto e le cifre successive
                    num.push(chars.next().unwrap()); // '.'
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            num.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    result.push_str(&num);
                } else {
                    // Intero, aggiungi ".0"
                    result.push_str(&num);
                    result.push_str(".0");
                }
            } else {
                // Fine della stringa, intero
                result.push_str(&num);
                result.push_str(".0");
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Valuta un'espressione aritmetica.
/// Se l'espressione completa non è valida, prova a valutare il prefisso più lungo possibile.
/// Restituisce una stringa formattata con il risultato (es. "= 42" o "= 1.4").
pub fn eval_expression(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }

    // Formatta un valore f64 in modo leggibile
    fn format_result(value: f64) -> String {
        if value.fract().abs() < 1e-10 {
            format!("= {}", value.round() as i64)
        } else {
            let s = format!("{:.6}", value);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            format!("= {}", s)
        }
    }

    // Valuta con preprocessing e converte in f64
    fn eval_preprocessed(s: &str) -> Option<f64> {
        let pre = ensure_float_literals(s);
        match eval(&pre) {
            Ok(Value::Int(i)) => Some(i as f64),
            Ok(Value::Float(f)) => Some(f),
            Ok(Value::Boolean(b)) => Some(if b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    // Prova l'espressione intera
    if let Some(val) = eval_preprocessed(expr) {
        return Some(format_result(val));
    }

    // Se fallisce, togli caratteri dalla fine finché non si ottiene un'espressione valida
    let mut truncated = expr.to_string();
    while !truncated.is_empty() {
        truncated.pop();
        if truncated.is_empty() {
            break;
        }
        if let Some(val) = eval_preprocessed(&truncated) {
            return Some(format_result(val));
        }
    }

    None
}
