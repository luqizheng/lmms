//! Ocelot 风格路径模板：字面量与 {var} 交替，提取 dict 后做朴素 replace。

use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TplPart {
    Literal(String),
    Var(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub parts: Vec<TplPart>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("unclosed '{{' in template")]
    UnclosedBrace,
    #[error("empty variable name")]
    EmptyVarName,
    #[error("two variables must be separated by a non-empty literal")]
    AdjacentVars,
    #[error("duplicate variable name: {0}")]
    DuplicateVar(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExtractError {
    #[error("literal '{0}' not matched at cursor {1}")]
    LiteralMismatch(String, usize),
    #[error("anchor literal '{0}' not found after variable")]
    AnchorNotFound(String),
    #[error("trailing input after template consumed")]
    TrailingInput,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("missing variable in dict: {0}")]
    MissingVar(String),
}

impl Template {
    pub fn parse(input: &str) -> Result<Self, TemplateError> {
        let bytes = input.as_bytes();
        let mut parts: Vec<TplPart> = Vec::new();
        let mut buf = String::new();
        let mut i = 0;
        let mut seen: HashSet<String> = HashSet::new();
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c == '{' {
                if !buf.is_empty() {
                    parts.push(TplPart::Literal(std::mem::take(&mut buf)));
                } else if matches!(parts.last(), Some(TplPart::Var(_))) {
                    return Err(TemplateError::AdjacentVars);
                }
                let end = input[i + 1..]
                    .find('}')
                    .ok_or(TemplateError::UnclosedBrace)?
                    + i
                    + 1;
                let name = &input[i + 1..end];
                if name.is_empty() {
                    return Err(TemplateError::EmptyVarName);
                }
                if !seen.insert(name.to_string()) {
                    return Err(TemplateError::DuplicateVar(name.to_string()));
                }
                parts.push(TplPart::Var(name.to_string()));
                i = end + 1;
            } else {
                buf.push(c);
                i += 1;
            }
        }
        if !buf.is_empty() {
            parts.push(TplPart::Literal(buf));
        }
        Ok(Self { parts })
    }

    pub fn var_names(&self) -> Vec<String> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                TplPart::Var(n) => Some(n.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn extract(&self, input: &str) -> Result<HashMap<String, String>, ExtractError> {
        let mut dict = HashMap::new();
        let mut cursor = 0usize;
        for (idx, part) in self.parts.iter().enumerate() {
            match part {
                TplPart::Literal(s) => {
                    if !input[cursor..].starts_with(s.as_str()) {
                        return Err(ExtractError::LiteralMismatch(s.clone(), cursor));
                    }
                    cursor += s.len();
                }
                TplPart::Var(name) => {
                    let next_lit = self.parts[idx + 1..].iter().find_map(|p| match p {
                        TplPart::Literal(s) => Some(s.as_str()),
                        _ => None,
                    });
                    let end = match next_lit {
                        Some(lit) => input[cursor..]
                            .find(lit)
                            .map(|off| cursor + off)
                            .ok_or_else(|| ExtractError::AnchorNotFound(lit.to_string()))?,
                        None => input.len(),
                    };
                    dict.insert(name.clone(), input[cursor..end].to_string());
                    cursor = end;
                }
            }
        }
        if cursor != input.len() {
            return Err(ExtractError::TrailingInput);
        }
        Ok(dict)
    }

    pub fn render(&self, dict: &HashMap<String, String>) -> Result<String, RenderError> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                TplPart::Literal(s) => out.push_str(s),
                TplPart::Var(n) => {
                    let v = dict
                        .get(n)
                        .ok_or_else(|| RenderError::MissingVar(n.clone()))?;
                    out.push_str(v);
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_literal_only() {
        let t = Template::parse("/iam/auth/login").unwrap();
        assert_eq!(t.parts, vec![TplPart::Literal("/iam/auth/login".into())]);
    }

    #[test]
    fn parse_one_var_at_tail() {
        let t = Template::parse("/v1/iam/{rest}").unwrap();
        assert_eq!(
            t.parts,
            vec![
                TplPart::Literal("/v1/iam/".into()),
                TplPart::Var("rest".into()),
            ]
        );
    }

    #[test]
    fn parse_multi_vars_with_separator() {
        let t = Template::parse("/a/{x}/b/{y}").unwrap();
        assert_eq!(
            t.parts,
            vec![
                TplPart::Literal("/a/".into()),
                TplPart::Var("x".into()),
                TplPart::Literal("/b/".into()),
                TplPart::Var("y".into()),
            ]
        );
    }

    #[test]
    fn parse_rejects_adjacent_vars() {
        let err = Template::parse("/a/{x}{y}").unwrap_err();
        assert!(matches!(err, TemplateError::AdjacentVars));
    }

    #[test]
    fn parse_rejects_unclosed_brace() {
        assert!(matches!(
            Template::parse("/a/{x").unwrap_err(),
            TemplateError::UnclosedBrace
        ));
    }

    #[test]
    fn parse_rejects_empty_var_name() {
        assert!(matches!(
            Template::parse("/a/{}/b").unwrap_err(),
            TemplateError::EmptyVarName
        ));
    }

    #[test]
    fn parse_rejects_duplicate_var_name() {
        assert!(matches!(
            Template::parse("/a/{x}/b/{x}").unwrap_err(),
            TemplateError::DuplicateVar(_)
        ));
    }
}
