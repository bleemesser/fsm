use anyhow::{Result, anyhow};
use bimap::BiMap;
use std::collections::{BTreeMap, BTreeSet};
use std::iter::Peekable;
use std::str::Chars;

use crate::yaml_parser::{Fsm, Nfa};

/// Represents a regular expression as a recursively defined data structure.
#[derive(Debug, PartialEq, Clone)]
pub enum Expression {
    /// An empty string
    Epsilon,
    /// A single literal character
    Literal(char),
    /// A sequence of expressions (e.g., `ab`)
    Concat(Box<Expression>, Box<Expression>),
    /// A choice between two expressions (e.g., `a|b`)
    Alternate(Box<Expression>, Box<Expression>),
    /// Zero or more repetitions of an expression (e.g., `a*`)
    Star(Box<Expression>),
}

/// A temporary representation of an NFA used during construction.
#[derive(Debug)]
struct NfaBuilder {
    transitions: BTreeMap<(usize, Option<char>), BTreeSet<usize>>,
    state_counter: usize,
}

impl NfaBuilder {
    fn new() -> Self {
        NfaBuilder {
            transitions: BTreeMap::new(),
            state_counter: 0,
        }
    }

    fn new_state(&mut self) -> usize {
        let state = self.state_counter;
        self.state_counter += 1;
        state
    }

    fn add_transition(&mut self, from: usize, to: usize, on: Option<char>) {
        self.transitions.entry((from, on)).or_default().insert(to);
    }
}

/// Converts a regex string into a runnable `Fsm`.
/// This supports a very specific set of syntax.
/// The supported syntax is:
/// - Literals: a-z, A-Z, 0-9
/// - Concatenation: ab (a followed by b)
/// - Alternation (union): a|b (a or b)
/// - Kleene star: a* (zero or more occurrences of a)
/// - Grouping: (ab|cd)* (zero or more occurrences of ab or cd)
///
/// Also supports shorthands:
/// - Plus: a+ (one or more occurrences of a, equiv to aa*)
/// - Exponentiation: (ab)^3 (exactly 3 occurrences of ab, equiv to ababab)
/// - Optional: a? (zero or one occurrence of a, equiv to (a|ε))
pub fn from_regex(regex: &str) -> Result<Fsm> {
    let start = std::time::Instant::now();
    let expr = parse(regex)?;
    let duration = start.elapsed();
    println!("Parsed regex in: {:.2?}", duration);
    let mut builder = NfaBuilder::new();

    let start = std::time::Instant::now();
    let (start_state, accept_state) = expr_to_nfa(&expr, &mut builder);

    let mut nfa_state_keys = BiMap::new();
    for i in 0..builder.state_counter {
        nfa_state_keys.insert(format!("q{}", i), i);
    }

    let nfa = Nfa {
        transitions: builder.transitions,
        start_state,
        nfa_accept_states: BTreeSet::from([accept_state]),
        nfa_state_keys,
    };
    let duration = start.elapsed();
    println!("Constructed NFA in: {:.2?}", duration);

    let alphabet_set = nfa
        .transitions
        .keys()
        .filter_map(|(_, c)| *c)
        .collect::<BTreeSet<char>>();

    let name = format!("regex: {}", regex);
    let start = std::time::Instant::now();
    let dfa = nfa.clone().to_dfa(&name, None, &alphabet_set)?;
    let duration = start.elapsed();
    println!("Converted NFA to DFA in: {:.2?}", duration);

    Ok(Fsm::Nfa { dfa, nfa })
}

/// Recursively converts an `Expression` into an NFA using Thompson's construction.
fn expr_to_nfa(expr: &Expression, builder: &mut NfaBuilder) -> (usize, usize) {
    match expr {
        Expression::Epsilon => {
            let start = builder.new_state();
            let end = builder.new_state();
            builder.add_transition(start, end, None);
            (start, end)
        }
        Expression::Literal(c) => {
            let start = builder.new_state();
            let end = builder.new_state();
            builder.add_transition(start, end, Some(*c));
            (start, end)
        }
        Expression::Concat(left, right) => {
            let (left_start, left_end) = expr_to_nfa(left, builder);
            let (right_start, right_end) = expr_to_nfa(right, builder);
            builder.add_transition(left_end, right_start, None); // epsilon transition
            (left_start, right_end)
        }
        Expression::Alternate(left, right) => {
            let start = builder.new_state();
            let end = builder.new_state();
            let (left_start, left_end) = expr_to_nfa(left, builder);
            let (right_start, right_end) = expr_to_nfa(right, builder);
            builder.add_transition(start, left_start, None);
            builder.add_transition(start, right_start, None);
            builder.add_transition(left_end, end, None);
            builder.add_transition(right_end, end, None);
            (start, end)
        }
        Expression::Star(expr) => {
            let start = builder.new_state();
            let end = builder.new_state();
            let (expr_start, expr_end) = expr_to_nfa(expr, builder);
            builder.add_transition(start, end, None); // epsilon transition for zero occurrences
            builder.add_transition(start, expr_start, None);
            builder.add_transition(expr_end, end, None);
            builder.add_transition(expr_end, expr_start, None); // full loop
            (start, end)
        }
    }
}

/// Parses a raw string into a regular expression.
fn parse(raw: &str) -> Result<Expression> {
    if raw.is_empty() {
        return Err(anyhow!("Empty regex string"));
    }

    let cleaned = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let mut chars = cleaned.chars().peekable();

    let expr = parse_alternate(&mut chars)?;

    if chars.next().is_some() {
        Err(anyhow!("Unexpected token after parsed expression"))
    } else {
        Ok(expr)
    }
}

fn parse_alternate(chars: &mut Peekable<Chars>) -> Result<Expression> {
    let mut left = parse_concat(chars)?;

    while let Some('|') = chars.peek() {
        chars.next(); // Consume the '|'
        let right = parse_concat(chars)?;
        left = Expression::Alternate(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn parse_concat(chars: &mut Peekable<Chars>) -> Result<Expression> {
    let mut left = parse_postfix(chars)?;

    // if next token can start an expression, it's concatenation
    while let Some(&c) = chars.peek() {
        if c != ')' && c != '|' {
            let right = parse_postfix(chars)?;
            left = Expression::Concat(Box::new(left), Box::new(right));
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_postfix(chars: &mut Peekable<Chars>) -> Result<Expression> {
    let mut expr = parse_term(chars)?;

    while let Some(&c) = chars.peek() {
        match c {
            '*' => {
                chars.next();
                expr = Expression::Star(Box::new(expr));
            }
            '+' => {
                chars.next();
                expr = Expression::Concat(
                    Box::new(expr.clone()),
                    Box::new(Expression::Star(Box::new(expr))),
                );
            }
            '?' => {
                chars.next();
                expr = Expression::Alternate(Box::new(expr), Box::new(Expression::Epsilon));
            }
            '^' => {
                chars.next();

                let mut num_str = String::new();
                while let Some(digit @ '0'..='9') = chars.peek().cloned() {
                    num_str.push(digit);
                    chars.next();
                }

                if num_str.is_empty() {
                    return Err(anyhow!("Expected a number after '^' for exponentiation."));
                }

                let n: u32 = num_str
                    .parse()
                    .map_err(|_| anyhow!("Invalid number for exponent"))?;

                if n == 0 {
                    return Err(anyhow!("Exponent must be a positive integer."));
                }

                if n > 1 {
                    let base_expr = expr.clone();
                    for _ in 2..=n {
                        expr = Expression::Concat(Box::new(expr), Box::new(base_expr.clone()));
                    }
                }
            }
            _ => break,
        }
    }
    Ok(expr)
}

fn parse_term(chars: &mut Peekable<Chars>) -> Result<Expression> {
    if let Some(c) = chars.next() {
        match c {
            '(' => {
                let expr = parse_alternate(chars)?;
                if chars.next() != Some(')') {
                    return Err(anyhow!("Mismatched parentheses: expected ')'"));
                }
                Ok(expr)
            }
            ')' => Err(anyhow!("Mismatched parentheses: unexpected ')'")),
            '|' | '*' | '+' | '?' | '^' => Err(anyhow!("Unexpected operator: '{}'", c)),
            _ => Ok(Expression::Literal(c)),
        }
    } else {
        Err(anyhow!("Unexpected end of expression"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! lit {
        ($c:expr) => {
            Box::new(Expression::Literal($c))
        };
    }

    macro_rules! eps {
        () => {
            Box::new(Expression::Epsilon)
        };
    }

    macro_rules! concat {
        ($left:expr, $right:expr) => {
            Box::new(Expression::Concat($left, $right))
        };
    }

    macro_rules! alt {
        ($left:expr, $right:expr) => {
            Box::new(Expression::Alternate($left, $right))
        };
    }

    macro_rules! star {
        ($expr:expr) => {
            Box::new(Expression::Star($expr))
        };
    }

    #[test]
    fn test_parse_literal() {
        let expr = parse("a").unwrap();
        assert_eq!(*lit!('a'), expr);
    }

    #[test]
    fn test_parse_concatenation() {
        let expr = parse("ab").unwrap();
        assert_eq!(*concat!(lit!('a'), lit!('b')), expr);
    }

    #[test]
    fn test_parse_alternation() {
        let expr = parse("a|b").unwrap();
        assert_eq!(*alt!(lit!('a'), lit!('b')), expr);
    }

    #[test]
    fn test_parse_kleene_star() {
        let expr = parse("a*").unwrap();
        assert_eq!(*star!(lit!('a')), expr);
    }

    #[test]
    fn test_parse_grouping() {
        let expr = parse("(a|b)*").unwrap();
        let inner = alt!(lit!('a'), lit!('b'));
        assert_eq!(*star!(inner), expr);
    }

    #[test]
    fn test_parse_plus() {
        let expr = parse("a+").unwrap();
        let expected = concat!(lit!('a'), star!(lit!('a')));
        assert_eq!(*expected, expr);
    }

    #[test]
    fn test_parse_optional() {
        let expr = parse("a?").unwrap();
        let expected = alt!(lit!('a'), eps!());
        assert_eq!(*expected, expr);
    }

    #[test]
    fn test_parse_complex_concatenation() {
        let expr = parse("a(b|c)d").unwrap();
        let b_or_c = alt!(lit!('b'), lit!('c'));
        let a_then_rest = concat!(lit!('a'), b_or_c);
        let final_expr = concat!(a_then_rest, lit!('d'));
        assert_eq!(*final_expr, expr);
    }

    #[test]
    fn test_parse_complex_alternation() {
        let expr = parse("ab|cd").unwrap();
        let ab = concat!(lit!('a'), lit!('b'));
        let cd = concat!(lit!('c'), lit!('d'));
        assert_eq!(*alt!(ab, cd), expr);
    }

    #[test]
    fn test_parse_nested_groups() {
        let expr = parse("(a(b|c)*)+").unwrap();
        let b_or_c_star = star!(alt!(lit!('b'), lit!('c')));
        let inner = concat!(lit!('a'), b_or_c_star);
        let expected = concat!(inner.clone(), star!(inner));
        assert_eq!(*expected, expr);
    }

    #[test]
    fn test_parse_exponentiation() {
        let expr = parse("(ab)^3").unwrap();
        let ab = concat!(lit!('a'), lit!('b'));
        let ab3 = concat!(concat!(ab.clone(), ab.clone()), ab);
        assert_eq!(*ab3, expr);
    }
}
