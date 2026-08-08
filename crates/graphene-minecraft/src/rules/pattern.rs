use crate::error::mc_error;
use graphene_core::{ErrorCode, GrapheneError, Result};

const MAX_RULE_PATTERN_LEN: usize = 256;

pub(super) fn safe_pattern_matches(pattern: &str, value: &str) -> Result<bool> {
    if pattern.is_empty() || pattern.len() > MAX_RULE_PATTERN_LEN || value.len() > 1024 {
        return Err(mc_error(
            ErrorCode::MinecraftRuleInvalid,
            "OS version regex or input exceeds its bound",
        ));
    }

    let expression = RegexParser::new(pattern).parse()?;
    let program = compile_regex(&expression)?;
    Ok(run_regex(&program, value))
}

#[derive(Debug, Clone)]
enum RegexExpr {
    Empty,
    Atom(RegexMatcher),
    AssertStart,
    AssertEnd,
    Sequence(Vec<RegexExpr>),
    Alternation(Vec<RegexExpr>),
    ZeroOrMore(Box<RegexExpr>),
    OneOrMore(Box<RegexExpr>),
    Optional(Box<RegexExpr>),
}

#[derive(Debug, Clone)]
enum RegexMatcher {
    Literal(char),
    Any,
    Digit(bool),
    Word(bool),
    Space(bool),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl RegexMatcher {
    fn matches(&self, value: char) -> bool {
        match self {
            Self::Literal(expected) => value == *expected,
            Self::Any => value != '\n' && value != '\r',
            Self::Digit(positive) => value.is_ascii_digit() == *positive,
            Self::Word(positive) => (value.is_ascii_alphanumeric() || value == '_') == *positive,
            Self::Space(positive) => value.is_ascii_whitespace() == *positive,
            Self::Class { negated, ranges } => {
                let matched = ranges
                    .iter()
                    .any(|(start, end)| *start <= value && value <= *end);
                matched != *negated
            }
        }
    }
}

struct RegexParser {
    chars: Vec<char>,
    cursor: usize,
}

impl RegexParser {
    fn new(pattern: &str) -> Self {
        Self {
            chars: pattern.chars().collect(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<RegexExpr> {
        let expression = self.parse_alternation()?;
        if self.cursor != self.chars.len() {
            return Err(regex_error(
                "OS version regex contains an unmatched closing delimiter",
            ));
        }

        Ok(expression)
    }

    fn parse_alternation(&mut self) -> Result<RegexExpr> {
        let mut branches = vec![self.parse_sequence()?];
        while self.peek() == Some('|') {
            self.cursor += 1;
            branches.push(self.parse_sequence()?);
        }

        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            RegexExpr::Alternation(branches)
        })
    }

    fn parse_sequence(&mut self) -> Result<RegexExpr> {
        let mut expressions = Vec::new();
        while !matches!(self.peek(), None | Some(')') | Some('|')) {
            expressions.push(self.parse_repetition()?);
        }

        Ok(match expressions.len() {
            0 => RegexExpr::Empty,
            1 => expressions.remove(0),
            _ => RegexExpr::Sequence(expressions),
        })
    }

    fn parse_repetition(&mut self) -> Result<RegexExpr> {
        let mut expression = self.parse_atom()?;
        if let Some(quantifier @ ('*' | '+' | '?')) = self.peek() {
            self.cursor += 1;
            expression = match quantifier {
                '*' => RegexExpr::ZeroOrMore(Box::new(expression)),
                '+' => RegexExpr::OneOrMore(Box::new(expression)),
                '?' => RegexExpr::Optional(Box::new(expression)),
                _ => {
                    return Err(regex_error(
                        "OS version regex contains an invalid quantifier",
                    ));
                }
            };
            if matches!(self.peek(), Some('*' | '+' | '?')) {
                return Err(regex_error(
                    "OS version regex contains repeated quantifiers",
                ));
            }
        }

        Ok(expression)
    }

    fn parse_atom(&mut self) -> Result<RegexExpr> {
        let character = self
            .next()
            .ok_or_else(|| regex_error("OS version regex ended unexpectedly"))?;
        match character {
            '(' => {
                let expression = self.parse_alternation()?;
                if self.next() != Some(')') {
                    return Err(regex_error("OS version regex contains an unmatched group"));
                }
                Ok(expression)
            }
            ')' | '|' | '*' | '+' | '?' => Err(regex_error(
                "OS version regex contains a misplaced operator",
            )),
            '[' => self.parse_class().map(RegexExpr::Atom),
            ']' => Err(regex_error(
                "OS version regex contains an unmatched class delimiter",
            )),
            '{' | '}' => Err(regex_error(
                "counted OS version regex repetitions are unsupported",
            )),
            '^' => Ok(RegexExpr::AssertStart),
            '$' => Ok(RegexExpr::AssertEnd),
            '.' => Ok(RegexExpr::Atom(RegexMatcher::Any)),
            '\\' => self.parse_escape().map(RegexExpr::Atom),
            literal => Ok(RegexExpr::Atom(RegexMatcher::Literal(literal))),
        }
    }

    fn parse_escape(&mut self) -> Result<RegexMatcher> {
        let escaped = self
            .next()
            .ok_or_else(|| regex_error("OS version regex ends with an escape"))?;

        Ok(match escaped {
            'd' => RegexMatcher::Digit(true),
            'D' => RegexMatcher::Digit(false),
            'w' => RegexMatcher::Word(true),
            'W' => RegexMatcher::Word(false),
            's' => RegexMatcher::Space(true),
            'S' => RegexMatcher::Space(false),
            other => RegexMatcher::Literal(other),
        })
    }

    fn parse_class(&mut self) -> Result<RegexMatcher> {
        let negated = if self.peek() == Some('^') {
            self.cursor += 1;
            true
        } else {
            false
        };

        let mut ranges = Vec::<(char, char)>::new();
        let mut first = true;

        while let Some(character) = self.peek() {
            if character == ']' && !first {
                self.cursor += 1;
                if ranges.is_empty() {
                    return Err(regex_error(
                        "OS version regex contains an empty character class",
                    ));
                }
                return Ok(RegexMatcher::Class { negated, ranges });
            }

            first = false;
            let start = self.parse_class_character()?;
            if self.peek() == Some('-')
                && self
                    .chars
                    .get(self.cursor + 1)
                    .is_some_and(|value| *value != ']')
            {
                self.cursor += 1;
                let end = self.parse_class_character()?;
                if start > end {
                    return Err(regex_error(
                        "OS version regex contains a reversed character range",
                    ));
                }
                ranges.push((start, end));
            } else {
                ranges.push((start, start));
            }
        }

        Err(regex_error(
            "OS version regex contains an unterminated character class",
        ))
    }

    fn parse_class_character(&mut self) -> Result<char> {
        let character = self
            .next()
            .ok_or_else(|| regex_error("OS version regex character class ended unexpectedly"))?;
        if character == '\\' {
            let escaped = self.next().ok_or_else(|| {
                regex_error("OS version regex character class ends with an escape")
            })?;

            if matches!(escaped, 'd' | 'D' | 'w' | 'W' | 's' | 'S') {
                return Err(regex_error(
                    "character-class shorthand escapes are unsupported in OS version regexes",
                ));
            }
            Ok(escaped)
        } else {
            Ok(character)
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }
    fn next(&mut self) -> Option<char> {
        let value = self.peek()?;
        self.cursor += 1;
        Some(value)
    }
}

#[derive(Debug, Clone)]
enum RegexInstruction {
    Consume(RegexMatcher, Option<usize>),
    Split(Option<usize>, Option<usize>),
    Jump(Option<usize>),
    AssertStart(Option<usize>),
    AssertEnd(Option<usize>),
    Accept,
}

#[derive(Debug)]
struct RegexFragment {
    start: usize,
    outs: Vec<(usize, u8)>,
}

fn compile_regex(expression: &RegexExpr) -> Result<Vec<RegexInstruction>> {
    // Instruction zero is a stable entry point even when a Thompson fragment's start is a split
    // emitted after one or more child fragments.
    let mut program = vec![RegexInstruction::Jump(None)];
    let fragment = compile_regex_expr(expression, &mut program)?;
    if program.len() > MAX_RULE_PATTERN_LEN.saturating_mul(8) {
        return Err(regex_error(
            "OS version regex expands beyond its program bound",
        ));
    }

    patch_regex_outs(&mut program, &[(0, 0)], fragment.start)?;
    let accept = program.len();
    program.push(RegexInstruction::Accept);
    patch_regex_outs(&mut program, &fragment.outs, accept)?;

    Ok(program)
}

fn compile_regex_expr(
    expression: &RegexExpr,
    program: &mut Vec<RegexInstruction>,
) -> Result<RegexFragment> {
    match expression {
        RegexExpr::Empty => {
            let index = program.len();
            program.push(RegexInstruction::Jump(None));
            Ok(RegexFragment {
                start: index,
                outs: vec![(index, 0)],
            })
        }
        RegexExpr::Atom(matcher) => {
            let index = program.len();
            program.push(RegexInstruction::Consume(matcher.clone(), None));
            Ok(RegexFragment {
                start: index,
                outs: vec![(index, 0)],
            })
        }
        RegexExpr::AssertStart => {
            let index = program.len();
            program.push(RegexInstruction::AssertStart(None));
            Ok(RegexFragment {
                start: index,
                outs: vec![(index, 0)],
            })
        }
        RegexExpr::AssertEnd => {
            let index = program.len();
            program.push(RegexInstruction::AssertEnd(None));
            Ok(RegexFragment {
                start: index,
                outs: vec![(index, 0)],
            })
        }
        RegexExpr::Sequence(expressions) => {
            let mut iterator = expressions.iter();
            let Some(first) = iterator.next() else {
                return compile_regex_expr(&RegexExpr::Empty, program);
            };
            let mut fragment = compile_regex_expr(first, program)?;
            for expression in iterator {
                let next = compile_regex_expr(expression, program)?;
                patch_regex_outs(program, &fragment.outs, next.start)?;
                fragment.outs = next.outs;
            }
            Ok(fragment)
        }
        RegexExpr::Alternation(expressions) => {
            let mut iterator = expressions.iter();
            let Some(first) = iterator.next() else {
                return compile_regex_expr(&RegexExpr::Empty, program);
            };
            let mut fragment = compile_regex_expr(first, program)?;
            for expression in iterator {
                let right = compile_regex_expr(expression, program)?;
                let split = program.len();
                program.push(RegexInstruction::Split(
                    Some(fragment.start),
                    Some(right.start),
                ));
                fragment = RegexFragment {
                    start: split,
                    outs: fragment.outs.into_iter().chain(right.outs).collect(),
                };
            }
            Ok(fragment)
        }
        RegexExpr::ZeroOrMore(expression) => {
            let child = compile_regex_expr(expression, program)?;
            let split = program.len();
            program.push(RegexInstruction::Split(Some(child.start), None));
            patch_regex_outs(program, &child.outs, split)?;
            Ok(RegexFragment {
                start: split,
                outs: vec![(split, 1)],
            })
        }
        RegexExpr::OneOrMore(expression) => {
            let child = compile_regex_expr(expression, program)?;
            let split = program.len();
            program.push(RegexInstruction::Split(Some(child.start), None));
            patch_regex_outs(program, &child.outs, split)?;
            Ok(RegexFragment {
                start: child.start,
                outs: vec![(split, 1)],
            })
        }
        RegexExpr::Optional(expression) => {
            let child = compile_regex_expr(expression, program)?;
            let split = program.len();
            program.push(RegexInstruction::Split(Some(child.start), None));
            let mut outs = child.outs;
            outs.push((split, 1));
            Ok(RegexFragment { start: split, outs })
        }
    }
}

fn patch_regex_outs(
    program: &mut [RegexInstruction],
    outs: &[(usize, u8)],
    target: usize,
) -> Result<()> {
    for &(index, slot) in outs {
        let instruction = program
            .get_mut(index)
            .ok_or_else(|| regex_error("OS version regex compiler produced an invalid patch"))?;
        match (instruction, slot) {
            (RegexInstruction::Consume(_, next), 0)
            | (RegexInstruction::Jump(next), 0)
            | (RegexInstruction::AssertStart(next), 0)
            | (RegexInstruction::AssertEnd(next), 0) => *next = Some(target),
            (RegexInstruction::Split(left, _), 0) => *left = Some(target),
            (RegexInstruction::Split(_, right), 1) => *right = Some(target),
            _ => {
                return Err(regex_error(
                    "OS version regex compiler produced an invalid patch slot",
                ));
            }
        }
    }

    Ok(())
}

fn run_regex(program: &[RegexInstruction], value: &str) -> bool {
    let characters = value.chars().collect::<Vec<_>>();
    let mut current = Vec::<usize>::new();
    let mut visited = vec![false; program.len()];

    add_regex_state(program, 0, 0, characters.len(), &mut current, &mut visited);
    for (position, character) in characters.iter().copied().enumerate() {
        let mut next = Vec::<usize>::new();
        let mut next_visited = vec![false; program.len()];
        for state in current {
            if let RegexInstruction::Consume(matcher, Some(target)) = &program[state]
                && matcher.matches(character)
            {
                add_regex_state(
                    program,
                    *target,
                    position + 1,
                    characters.len(),
                    &mut next,
                    &mut next_visited,
                );
            }
        }

        current = next;
        if current.is_empty() {
            return false;
        }
    }
    current
        .into_iter()
        .any(|state| matches!(program[state], RegexInstruction::Accept))
}

fn add_regex_state(
    program: &[RegexInstruction],
    start: usize,
    position: usize,
    input_len: usize,
    output: &mut Vec<usize>,
    visited: &mut [bool],
) {
    let mut stack = vec![start];
    while let Some(state) = stack.pop() {
        if state >= program.len() || std::mem::replace(&mut visited[state], true) {
            continue;
        }

        match &program[state] {
            RegexInstruction::Split(left, right) => {
                if let Some(right) = right {
                    stack.push(*right);
                }
                if let Some(left) = left {
                    stack.push(*left);
                }
            }
            RegexInstruction::Jump(Some(next)) => stack.push(*next),
            RegexInstruction::AssertStart(Some(next)) if position == 0 => stack.push(*next),
            RegexInstruction::AssertEnd(Some(next)) if position == input_len => stack.push(*next),
            RegexInstruction::Consume(_, _) | RegexInstruction::Accept => output.push(state),
            RegexInstruction::Jump(None)
            | RegexInstruction::AssertStart(None)
            | RegexInstruction::AssertEnd(None) => {}
            RegexInstruction::AssertStart(Some(_)) | RegexInstruction::AssertEnd(Some(_)) => {}
        }
    }
}

fn regex_error(message: &'static str) -> GrapheneError {
    mc_error(ErrorCode::MinecraftRuleInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_engine_is_bounded_and_supports_normal_rule_syntax() {
        assert!(safe_pattern_matches(r"^(10|11)\.[0-9]+$", "10.0").expect("regex"));
        assert!(safe_pattern_matches(r"^1?0\.\d+$", "10.22631").expect("regex"));
        assert!(!safe_pattern_matches(r"^10\.[0-9]+$", "11.0").expect("regex"));
        assert_eq!(
            safe_pattern_matches("(broken", "10")
                .expect_err("invalid")
                .code,
            ErrorCode::MinecraftRuleInvalid
        );
        assert_eq!(
            safe_pattern_matches("a{2}", "aa")
                .expect_err("unsupported")
                .code,
            ErrorCode::MinecraftRuleInvalid
        );
    }
}
