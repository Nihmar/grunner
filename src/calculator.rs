use meval;

pub fn eval_expression(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr.is_empty() {
        return None;
    }
    match meval::eval_str(expr) {
        Ok(result) => {
            // Formatta il risultato: intero senza decimali, altrimenti con al massimo 6 cifre decimali
            if result.fract().abs() < 1e-10 {
                Some(format!("= {}", result.round() as i64))
            } else {
                let s = format!("{:.6}", result);
                let s = s.trim_end_matches('0').trim_end_matches('.');
                Some(format!("= {}", s))
            }
        }
        Err(_) => None,
    }
}