//! Calculator evaluator for Grunner
//!
//! This module provides a simple calculator that evaluates mathematical expressions
//! entered in the default search mode. It serves as a fallback when GNOME's
//! calculator is blacklisted or not available as a smart provider.
//!
//! Features:
//! - Basic arithmetic operations (+, -, *, /, %)
//! - Parentheses for grouping
//! - Exponentiation (^)
//! - Floating point numbers
//! - Error handling for invalid expressions

use log::debug;

/// Evaluate a mathematical expression and return the result as a string
///
/// # Arguments
/// * `expr` - The mathematical expression to evaluate
///
/// # Returns
/// `Some(String)` with the evaluated result if successful,
/// `None` if the expression is invalid or cannot be evaluated.
///
/// # Examples
/// ```
/// # use grunner::calculator::evaluate;
/// assert_eq!(evaluate("2 + 2"), Some("4".to_string()));
/// assert_eq!(evaluate("10 * 5"), Some("50".to_string()));
/// assert_eq!(evaluate("(2 + 3) * 4"), Some("20".to_string()));
/// assert_eq!(evaluate("invalid"), None);
/// ```
#[must_use]
pub fn evaluate(expr: &str) -> Option<String> {
    // Trim whitespace
    let expr = expr.trim();

    // Empty expression
    if expr.is_empty() {
        return None;
    }

    // Check if expression contains only valid characters (numbers, operators, spaces, parentheses)
    if !expr.chars().all(|c| {
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
    }) {
        return None;
    }

    // Basic validation: expression should start with a digit, parenthesis, or unary minus
    let first_char = expr.chars().find(|c| !c.is_whitespace());
    if let Some(c) = first_char
        && !c.is_ascii_digit()
        && c != '('
        && c != '-'
    {
        return None;
    }

    // Check if expression contains at least one operator (to avoid evaluating simple numbers)
    // We need to be careful with minus signs - a leading minus might be unary, not binary
    let trimmed = expr.trim();
    let has_binary_op = trimmed
        .chars()
        .any(|c| matches!(c, '+' | '*' | '/' | '%' | '^'));
    let _has_minus = trimmed.contains('-');

    // Count how many minus signs there are and their positions
    let minus_positions: Vec<usize> = trimmed
        .char_indices()
        .filter(|(_, c)| *c == '-')
        .map(|(i, _)| i)
        .collect();

    // Check if there's a non-leading minus or multiple minuses
    let has_non_leading_minus = minus_positions.iter().any(|&pos| pos > 0);
    let has_multiple_minuses = minus_positions.len() > 1;

    // Allow evaluation if:
    // 1. Has any operator besides minus, OR
    // 2. Has a minus that's not just a leading unary minus, OR
    // 3. Has multiple minuses (e.g., "5 - -3")
    if has_binary_op || has_non_leading_minus || has_multiple_minuses {
        // Allow evaluation
    } else {
        return None;
    }

    debug!("Evaluating expression: {}", expr);

    // Parse and evaluate using shunting yard algorithm
    match evaluate_expression(expr) {
        Ok(result) => {
            debug!("Expression evaluated to: {}", result);
            Some(format_result(result))
        }
        Err(e) => {
            debug!("Failed to evaluate expression: {}", e);
            None
        }
    }
}

/// Format the result for display
///
/// Removes trailing zeros from floating point numbers
fn format_result(result: f64) -> String {
    if result == result.trunc() {
        // Integer result
        format!("{}", result as i64)
    } else {
        // Floating point result - remove trailing zeros
        let s = format!("{:.10}", result);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Evaluate an arithmetic expression using the shunting yard algorithm
///
/// # Errors
/// Returns an error if the expression is invalid or contains mismatched parentheses.
fn evaluate_expression(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    let rpn = shunting_yard(&tokens)?;
    evaluate_rpn(&rpn)
}

/// Tokenize the expression into numbers and operators
///
/// # Errors
/// Returns an error if the expression contains invalid characters or malformed numbers.
fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    let mut last_token_was_operator_or_open_paren = true; // Start with true for unary minus at beginning

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        if c.is_ascii_digit() || c == '.' {
            // Parse number
            let mut num_str = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() || ch == '.' {
                    num_str.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }

            // Validate number format
            if num_str.chars().filter(|&c| c == '.').count() > 1 {
                return Err(format!("Invalid number format: {}", num_str));
            }

            let num: f64 = num_str
                .parse()
                .map_err(|_| format!("Invalid number: {}", num_str))?;
            tokens.push(Token::Number(num));
            last_token_was_operator_or_open_paren = false;
        } else if c == '(' {
            tokens.push(Token::Operator(Operator::LeftParen));
            chars.next();
            last_token_was_operator_or_open_paren = true;
        } else if c == ')' {
            tokens.push(Token::Operator(Operator::RightParen));
            chars.next();
            last_token_was_operator_or_open_paren = false;
        } else if c == '-' && last_token_was_operator_or_open_paren {
            // Unary minus
            chars.next();
            tokens.push(Token::Operator(Operator::UnaryMinus));
            // Keep last_token_was_operator_or_open_paren as true to handle cases like "- -5"
            // but actually we want to allow another unary minus after this one
        } else if let Some(op) = match c {
            '+' => Some(Operator::Add),
            '-' => Some(Operator::Subtract), // This is a binary minus
            '*' => Some(Operator::Multiply),
            '/' => Some(Operator::Divide),
            '%' => Some(Operator::Modulo),
            '^' => Some(Operator::Power),
            _ => None,
        } {
            tokens.push(Token::Operator(op));
            chars.next();
            last_token_was_operator_or_open_paren = true;
        } else {
            return Err(format!("Unknown character: {}", c));
        }
    }

    Ok(tokens)
}

/// Token types for the calculator
#[derive(Debug, Clone, Copy)]
enum Token {
    Number(f64),
    Operator(Operator),
}

/// Operator types with precedence
#[derive(Debug, Clone, Copy, PartialEq)]
enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    UnaryMinus,
    LeftParen,
    RightParen,
}

impl Operator {
    /// Get the precedence level of an operator
    /// Higher number means higher precedence
    fn precedence(&self) -> u8 {
        match self {
            Operator::Add | Operator::Subtract => 1,
            Operator::Multiply | Operator::Divide | Operator::Modulo => 2,
            Operator::Power => 3,
            Operator::UnaryMinus => 4, // Unary minus has highest precedence
            Operator::LeftParen | Operator::RightParen => 0,
        }
    }

    /// Check if operator is left-associative
    fn is_left_associative(&self) -> bool {
        match self {
            Operator::Power | Operator::UnaryMinus => false, // Power and UnaryMinus are right-associative
            _ => true,
        }
    }
}

/// Convert infix expression to Reverse Polish Notation using shunting yard algorithm
///
/// # Errors
/// Returns an error if there are mismatched parentheses.
fn shunting_yard(tokens: &[Token]) -> Result<Vec<Token>, String> {
    let mut output = Vec::new();
    let mut stack = Vec::new();

    for &token in tokens {
        match token {
            Token::Number(_) => output.push(token),
            Token::Operator(op) => {
                if op == Operator::LeftParen {
                    stack.push(token);
                } else if op == Operator::RightParen {
                    // Pop until matching left paren
                    while let Some(&t) = stack.last() {
                        if let Token::Operator(Operator::LeftParen) = t {
                            stack.pop();
                            break;
                        }
                        output.push(stack.pop().unwrap());
                    }
                } else {
                    // Operator
                    while let Some(&t) = stack.last() {
                        if let Token::Operator(stack_op) = t {
                            if stack_op != Operator::LeftParen
                                && stack_op != Operator::RightParen
                                && (stack_op.precedence() > op.precedence()
                                    || (stack_op.precedence() == op.precedence()
                                        && op.is_left_associative()))
                            {
                                output.push(stack.pop().unwrap());
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    stack.push(token);
                }
            }
        }
    }

    // Pop remaining operators
    while let Some(token) = stack.pop() {
        if let Token::Operator(Operator::LeftParen) = token {
            return Err("Mismatched parentheses".to_string());
        }
        output.push(token);
    }

    Ok(output)
}

/// Evaluate a Reverse Polish Notation expression
///
/// # Errors
/// Returns an error if there are insufficient operands or division by zero.
fn evaluate_rpn(rpn: &[Token]) -> Result<f64, String> {
    let mut stack = Vec::new();

    for &token in rpn {
        match token {
            Token::Number(n) => stack.push(n),
            Token::Operator(op) => match op {
                Operator::UnaryMinus => {
                    if stack.is_empty() {
                        return Err("Insufficient operands for unary minus".to_string());
                    }
                    let a = stack.pop().unwrap();
                    stack.push(-a);
                }
                _ => {
                    if stack.len() < 2 {
                        return Err("Insufficient operands".to_string());
                    }
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    let result = match op {
                        Operator::Add => a + b,
                        Operator::Subtract => a - b,
                        Operator::Multiply => a * b,
                        Operator::Divide => {
                            if b == 0.0 {
                                return Err("Division by zero".to_string());
                            }
                            a / b
                        }
                        Operator::Modulo => a % b,
                        Operator::Power => a.powf(b),
                        _ => return Err("Invalid operator in RPN".to_string()),
                    };
                    stack.push(result);
                }
            },
        }
    }

    if stack.len() != 1 {
        return Err("Invalid expression".to_string());
    }

    Ok(stack[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(evaluate("2 + 2"), Some("4".to_string()));
        assert_eq!(evaluate("10 - 5"), Some("5".to_string()));
        assert_eq!(evaluate("3 * 4"), Some("12".to_string()));
        assert_eq!(evaluate("10 / 2"), Some("5".to_string()));
    }

    #[test]
    fn test_precedence() {
        assert_eq!(evaluate("2 + 3 * 4"), Some("14".to_string()));
        assert_eq!(evaluate("(2 + 3) * 4"), Some("20".to_string()));
    }

    #[test]
    fn test_floats() {
        assert_eq!(evaluate("10 / 3"), Some("3.3333333333".to_string()));
        assert_eq!(evaluate("0.5 + 0.5"), Some("1".to_string()));
    }

    #[test]
    fn test_invalid_expressions() {
        assert_eq!(evaluate("abc"), None);
        assert_eq!(evaluate("2 +"), None);
        assert_eq!(evaluate("+ 2"), None);
        assert_eq!(evaluate(""), None);
    }

    #[test]
    fn test_simple_numbers() {
        // Simple numbers without operators should not be evaluated
        assert_eq!(evaluate("42"), None);
        assert_eq!(evaluate("3.14"), None);
    }

    #[test]
    fn test_edge_cases() {
        // Test unary minus
        assert_eq!(evaluate("-5 + 3"), Some("-2".to_string()));
        assert_eq!(evaluate("5 + -3"), Some("2".to_string()));

        // Test exponentiation
        assert_eq!(evaluate("2 ^ 3"), Some("8".to_string()));

        // Test modulo
        assert_eq!(evaluate("10 % 3"), Some("1".to_string()));
    }
}
