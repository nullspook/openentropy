//! Source-name resolution helpers shared across CLI and SDK surfaces.

use std::collections::HashSet;

/// Matching policy used when resolving requested source names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMatchMode {
    /// Match only exact source names.
    ExactOnly,
    /// Match exact names first, then fall back to case-insensitive substring matching.
    ExactThenSubstringInsensitive,
}

/// Result of resolving user-requested source names against available source names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResolution {
    pub resolved: Vec<String>,
    pub missing: Vec<String>,
}

/// Resolve requested source names against an available source-name list.
///
/// Resolved names are returned in available-source order for determinism.
/// Duplicate requests for the same underlying source are silently deduplicated.
#[must_use]
pub fn resolve_source_names(
    available: &[String],
    requested: &[String],
    mode: SourceMatchMode,
) -> SourceResolution {
    let mut used_indices = HashSet::new();
    let mut missing = Vec::new();

    for name in requested {
        if let Some(idx) = find_matching_index(available, name, mode, Some(&used_indices)) {
            used_indices.insert(idx);
            continue;
        }

        // If this request resolves to a source that is already selected, treat it
        // as a duplicate alias/request instead of surfacing it as missing.
        if find_matching_index(available, name, mode, None).is_some() {
            continue;
        }

        missing.push(name.clone());
    }

    let mut indices: Vec<usize> = used_indices.into_iter().collect();
    indices.sort_unstable();
    let resolved = indices
        .into_iter()
        .map(|idx| available[idx].clone())
        .collect();

    SourceResolution { resolved, missing }
}

fn find_matching_index(
    available: &[String],
    requested: &str,
    mode: SourceMatchMode,
    used_indices: Option<&HashSet<usize>>,
) -> Option<usize> {
    let is_unused = |idx: &usize| used_indices.is_none_or(|used| !used.contains(idx));

    if let Some((idx, _)) = available
        .iter()
        .enumerate()
        .find(|(idx, source)| is_unused(idx) && source.as_str() == requested)
    {
        return Some(idx);
    }

    if mode == SourceMatchMode::ExactOnly {
        return None;
    }

    let lower = requested.to_lowercase();
    available
        .iter()
        .enumerate()
        .find(|(idx, source)| is_unused(idx) && source.to_lowercase().contains(&lower))
        .map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::{SourceMatchMode, resolve_source_names};

    fn available_sources() -> Vec<String> {
        vec![
            "clock_jitter".to_string(),
            "thermal_noise".to_string(),
            "mach_timing".to_string(),
        ]
    }

    #[test]
    fn exact_only_resolves_and_deduplicates() {
        let resolution = resolve_source_names(
            &available_sources(),
            &["thermal_noise".to_string(), "thermal_noise".to_string()],
            SourceMatchMode::ExactOnly,
        );

        assert_eq!(resolution.resolved, vec!["thermal_noise"]);
        assert!(resolution.missing.is_empty());
    }

    #[test]
    fn exact_only_reports_missing_names() {
        let resolution = resolve_source_names(
            &available_sources(),
            &["missing_source".to_string()],
            SourceMatchMode::ExactOnly,
        );

        assert!(resolution.resolved.is_empty());
        assert_eq!(resolution.missing, vec!["missing_source"]);
    }

    #[test]
    fn partial_mode_prefers_exact_then_partial() {
        let resolution = resolve_source_names(
            &available_sources(),
            &["mach".to_string(), "clock_jitter".to_string()],
            SourceMatchMode::ExactThenSubstringInsensitive,
        );

        assert_eq!(
            resolution.resolved,
            vec!["clock_jitter".to_string(), "mach_timing".to_string()]
        );
        assert!(resolution.missing.is_empty());
    }

    #[test]
    fn partial_mode_deduplicates_aliases_for_same_source() {
        let resolution = resolve_source_names(
            &available_sources(),
            &["clock".to_string(), "clock_jitter".to_string()],
            SourceMatchMode::ExactThenSubstringInsensitive,
        );

        assert_eq!(resolution.resolved, vec!["clock_jitter"]);
        assert!(resolution.missing.is_empty());
    }
}
