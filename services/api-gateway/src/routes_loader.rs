use crate::config::{RouteFile, RouteTable};
use crate::template::{Template, TemplateError};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("no .router.toml files in {0}")]
    Empty(String),
    #[error("parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("glob error: {0}")]
    Glob(#[from] glob::PatternError),
    #[error("duplicate upstream: {0}")]
    DuplicateUpstream(String),
    #[error("template error in {field} '{template}': {source}")]
    Template {
        field: &'static str,
        template: String,
        #[source]
        source: TemplateError,
    },
    #[error(
        "variable name set mismatch in route '{upstream}': upstream={up:?} downstream={down:?}"
    )]
    VarMismatch {
        upstream: String,
        up: Vec<String>,
        down: Vec<String>,
    },
}

pub fn load_routes(dir: &Path) -> Result<RouteTable, LoadError> {
    let pattern = dir.join("*.router.toml");
    let pattern_str = pattern
        .to_str()
        .ok_or_else(|| LoadError::Empty(dir.display().to_string()))?;

    println!("routes_loader.glob   : {}", pattern_str);
    let mut entries: Vec<_> = glob::glob(pattern_str)?.filter_map(|e| e.ok()).collect();
    entries.sort();
    println!("routes_loader.found  : {} file(s)", entries.len());
    for e in &entries {
        println!("routes_loader.entry  : {}", e.display());
    }

    let mut routes = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for entry in entries {
        let content = std::fs::read_to_string(&entry)?;
        let file: RouteFile = toml::from_str(&content).map_err(|e| LoadError::Parse {
            path: entry.display().to_string(),
            source: e,
        })?;
        for r in file.route {
            // 编译期校验 upstream/downstream
            let up = Template::parse(&r.upstream).map_err(|e| LoadError::Template {
                field: "upstream",
                template: r.upstream.clone(),
                source: e,
            })?;
            let down = Template::parse(&r.downstream).map_err(|e| LoadError::Template {
                field: "downstream",
                template: r.downstream.clone(),
                source: e,
            })?;
            let mut up_vars = up.var_names();
            let mut down_vars = down.var_names();
            up_vars.sort();
            down_vars.sort();
            if up_vars != down_vars {
                return Err(LoadError::VarMismatch {
                    upstream: r.upstream.clone(),
                    up: up_vars,
                    down: down_vars,
                });
            }
            if !seen.insert(r.upstream.clone()) {
                return Err(LoadError::DuplicateUpstream(r.upstream));
            }
            routes.push(r);
        }
    }

    // 字面量长度降序：更具体的优先匹配
    routes.sort_by_key(|b| std::cmp::Reverse(literal_len(&b.upstream)));

    Ok(RouteTable { routes })
}

fn literal_len(template: &str) -> usize {
    let mut depth: i32 = 0;
    let mut n = 0usize;
    for c in template.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ if depth == 0 => n += 1,
            _ => {}
        }
    }
    n
}
