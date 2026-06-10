use crate::config::{RouteFile, RouteTable};
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
    #[error("duplicate match pattern: {0}")]
    Duplicate(String),
}

pub fn load_routes(dir: &Path) -> Result<RouteTable, LoadError> {
    let pattern = dir.join("*.router.toml");
    let pattern_str = pattern
        .to_str()
        .ok_or_else(|| LoadError::Empty(dir.display().to_string()))?;

    let mut entries: Vec<_> = glob::glob(pattern_str)?.filter_map(|e| e.ok()).collect();
    entries.sort();

    if entries.is_empty() {
        return Err(LoadError::Empty(dir.display().to_string()));
    }

    let mut routes = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for entry in entries {
        let content = std::fs::read_to_string(&entry)?;
        let file: RouteFile = toml::from_str(&content).map_err(|e| LoadError::Parse {
            path: entry.display().to_string(),
            source: e,
        })?;
        for r in file.route {
            if !seen.insert(r.match_pattern.clone()) {
                return Err(LoadError::Duplicate(r.match_pattern));
            }
            routes.push(r);
        }
    }
    Ok(RouteTable { routes })
}
