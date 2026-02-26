//! Shared workspace path guards for filesystem tools.

use crate::error::ToolError;
use std::path::{Component, Path, PathBuf};

/// Return the canonical workspace root.
pub fn workspace_root(working_dir: &Path) -> Result<PathBuf, ToolError> {
    working_dir
        .canonicalize()
        .map_err(|e| ToolError::ExecutionError(format!("Cannot resolve working dir: {e}")))
}

/// Resolve an existing path and ensure it stays within workspace.
pub fn resolve_existing_path(path: &Path, working_dir: &Path) -> Result<PathBuf, ToolError> {
    let workspace = workspace_root(working_dir)?;
    let candidate = absolutize(path, &workspace);
    let canonical = candidate
        .canonicalize()
        .map_err(|e| ToolError::ExecutionError(format!("Cannot resolve path: {e}")))?;
    ensure_within_workspace(path, &canonical, &workspace)?;
    Ok(canonical)
}

/// Resolve a potentially-new path and ensure it stays within workspace.
pub fn resolve_path_for_write(path: &Path, working_dir: &Path) -> Result<PathBuf, ToolError> {
    let workspace = workspace_root(working_dir)?;
    let candidate = absolutize(path, &workspace);

    let resolved = if candidate.exists() {
        candidate
            .canonicalize()
            .map_err(|e| ToolError::ExecutionError(format!("Cannot resolve path: {e}")))?
    } else {
        let (existing_base, tail) = split_existing_ancestor(&candidate)?;
        existing_base.join(tail)
    };

    ensure_within_workspace(path, &resolved, &workspace)?;
    Ok(resolved)
}

fn absolutize(path: &Path, workspace: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(&workspace.join(path))
    }
}

fn ensure_within_workspace(original: &Path, resolved: &Path, workspace: &Path) -> Result<(), ToolError> {
    if !resolved.starts_with(workspace) {
        return Err(ToolError::PermissionDenied(format!(
            "Path '{}' is outside the working directory",
            original.display()
        )));
    }
    Ok(())
}

fn split_existing_ancestor(path: &Path) -> Result<(PathBuf, PathBuf), ToolError> {
    let mut existing = path.to_path_buf();
    let mut tail = PathBuf::new();

    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            ToolError::ExecutionError(format!("Cannot resolve path: {}", path.display()))
        })?;
        if tail.as_os_str().is_empty() {
            tail = PathBuf::from(name);
        } else {
            tail = PathBuf::from(name).join(&tail);
        }
        existing = existing.parent().ok_or_else(|| {
            ToolError::ExecutionError(format!("Cannot resolve path: {}", path.display()))
        })?
        .to_path_buf();
    }

    let canonical_existing = existing
        .canonicalize()
        .map_err(|e| ToolError::ExecutionError(format!("Cannot resolve path: {e}")))?;

    Ok((canonical_existing, tail))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out.push(component.as_os_str()),
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::Normal(seg) => out.push(seg),
        }
    }
    out
}
