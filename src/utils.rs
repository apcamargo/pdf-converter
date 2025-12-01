use std::collections::HashSet;
use std::path::Path;

const SEP: char = '-';

/// Internal sanitizer: keep ASCII alnum, '-', '_' and '.', replace others with '-'.
fn sanitize_prefix_raw(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_sep = false;
    for c in s.chars() {
        let keep = c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.';
        if keep {
            out.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push(SEP);
            last_was_sep = true;
        }
    }

    // Trim leading/trailing separators and collapse duplicates
    let trimmed = out.trim_matches(SEP).to_string();
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut collapsed = String::with_capacity(trimmed.len());
        let mut prev_sep = false;
        for ch in trimmed.chars() {
            if ch == SEP {
                if !prev_sep {
                    collapsed.push(SEP);
                    prev_sep = true;
                }
            } else {
                collapsed.push(ch);
                prev_sep = false;
            }
        }
        collapsed
    }
}

/// Resolve the filename prefix to use for output files.
///
/// - `prefix`: optional user-supplied prefix (`--prefix`)
/// - `input`: input `Path` used to infer stem when `prefix` is `None`
///
/// Behavior:
/// - If `prefix` is Some, it is sanitized then used.
/// - Otherwise, infer from `input.file_stem()` using `to_string_lossy`.
/// - If sanitization yields empty string, fall back to `"rendered"`.
/// - Ensure returned string ends with exactly one trailing separator `-`.
pub fn resolve_prefix(prefix: Option<&str>, input: &Path) -> String {
    let mut base = if let Some(p) = prefix {
        sanitize_prefix_raw(p)
    } else {
        let stem_lossy = input
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "rendered".to_string());
        sanitize_prefix_raw(&stem_lossy)
    };

    if base.is_empty() {
        base = "rendered".to_string();
    }
    if !base.ends_with(SEP) {
        base.push(SEP);
    }
    base
}

/// Validate requested 1-based page numbers against `total` pages in the document.
///
/// Returns:
/// - `Ok(HashSet<usize>)` with 0-based page indices when all requested pages are valid (may be empty).
/// - `Err(String)` with a single error message listing all problematic requested pages.
pub fn validate_requested_pages(
    requested: &[usize],
    total: usize,
) -> Result<HashSet<usize>, String> {
    if requested.is_empty() {
        return Ok(HashSet::new());
    }

    // Collect all problematic requested page numbers (1-based values):
    // - `0` is invalid because pages are 1-based
    // - any p where p - 1 >= total (i.e. p > total) is out-of-range
    let mut problematic: Vec<usize> = requested
        .iter()
        .copied()
        .filter(|&p| p == 0 || p > total)
        .collect();

    if !problematic.is_empty() {
        problematic.sort_unstable();
        problematic.dedup();
        let list = problematic
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let msg = format!(
            "Invalid requested page(s): {}. The page numbers must be between 1 and {}.",
            list, total
        );
        return Err(msg);
    }

    // All requested pages are valid; convert to 0-based set and return.
    let set: HashSet<usize> = requested.iter().map(|&p| p - 1).collect();
    Ok(set)
}
