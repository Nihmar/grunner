use evalexpr::*;

pub fn eval_expression(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }
    match eval(expr) {
        Ok(Value::Int(i)) => Some(format!("= {}", i)),
        Ok(Value::Float(f)) => {
            if f.fract().abs() < 1e-10 {
                Some(format!("= {}", f.round() as i64))
            } else {
                let s = format!("{:.6}", f);
                let s = s.trim_end_matches('0').trim_end_matches('.');
                Some(format!("= {}", s))
            }
        }
        _ => None,
    }
}
