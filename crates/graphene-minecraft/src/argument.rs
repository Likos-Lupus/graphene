use crate::{Rule, RuleContext, error::mc_error, rules_allow};
use graphene_core::{ErrorCode, Result};
use serde::{Deserialize, Serialize};

/// Argument templates remain vectors and are never interpreted by a shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Argument {
    Literal(String),
    Conditional {
        rules: Vec<Rule>,
        values: Vec<String>,
    },
}

impl Argument {
    /// Returns this argument's ordered values when its rules admit it.
    pub fn values_for(&self, context: &RuleContext) -> Result<Vec<&str>> {
        match self {
            Self::Literal(value) => Ok(vec![value]),
            Self::Conditional { rules, values } => {
                if rules_allow(rules, context)? {
                    Ok(values.iter().map(String::as_str).collect())
                } else {
                    Ok(Vec::new())
                }
            }
        }
    }
}

/// Deterministic shell-free tokenizer for legacy `minecraftArguments`.
pub fn tokenize_legacy_arguments(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut started = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            started = true;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            started = true;
            continue;
        }

        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
            started = true;
            continue;
        }

        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                started = true;
            }
            c if c.is_whitespace() => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(ch);
                started = true;
            }
        }
    }

    if escaped || quote.is_some() {
        return Err(mc_error(
            ErrorCode::MinecraftArgumentInvalid,
            "legacy Minecraft arguments contain malformed quoting or escaping",
        ));
    }

    if started {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManagedPath, MavenCoordinate};

    #[test]
    fn legacy_arguments_preserve_quoted_spaces_and_reject_malformed_input() {
        assert_eq!(
            tokenize_legacy_arguments(r#"--name "Player One" --demo 'two words' plain\ value"#)
                .expect("arguments"),
            vec!["--name", "Player One", "--demo", "two words", "plain value"]
        );
        assert_eq!(
            tokenize_legacy_arguments("--bad \"unterminated")
                .expect_err("malformed")
                .code,
            ErrorCode::MinecraftArgumentInvalid
        );
    }

    #[test]
    fn bounded_parser_property_inputs_do_not_panic() {
        let alphabet = b"abcXYZ019:/\\. _-+$[]()'\"";
        let mut state = 0x9e37_79b9_u32;
        for length in 0..96usize {
            let mut value = String::with_capacity(length);
            for _ in 0..length {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                value.push(alphabet[(state as usize) % alphabet.len()] as char);
            }
            let _ = MavenCoordinate::parse(&value);
            let _ = ManagedPath::new(value.clone());
            let _ = tokenize_legacy_arguments(&value);
        }
    }
}
