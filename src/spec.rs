use std::num::ParseIntError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRange {
    pub start: usize,        // 1-based inclusive
    pub end: Option<usize>,  // 1-based inclusive; None means open-ended
}

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("invalid number: {0}")]
    InvalidNumber(#[from] ParseIntError),
    #[error("invalid range segment: {0}")]
    InvalidSegment(String),
}

// Parse spec like: "1-3,5,10-" (1-based)
pub fn parse_spec(spec: &str) -> Result<Vec<PageRange>, SpecError> {
    let mut out = Vec::new();
    for raw in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Some((a, b)) = raw.split_once('-') {
            let start = if a.is_empty() { 1 } else { a.parse::<usize>()? };
            let end = if b.is_empty() { None } else { Some(b.parse::<usize>()?) };
            if let Some(e) = end { if e < start { return Err(SpecError::InvalidSegment(raw.to_string())); } }
            out.push(PageRange { start, end });
        } else {
            // single page
            let p = raw.parse::<usize>()?;
            out.push(PageRange { start: p, end: Some(p) });
        }
    }
    Ok(out)
}

// Expand to zero-based page indexes, deduped and sorted
pub fn expand_to_indexes(ranges: &[PageRange], total_pages: usize) -> Vec<usize> {
    use std::collections::BTreeSet;
    let mut set = BTreeSet::new();
    for r in ranges {
        if total_pages == 0 { break; }
        let start1 = r.start.max(1);
        let end1 = match r.end { Some(e) => e.min(total_pages), None => total_pages };
        if end1 < start1 { continue; }
        for p in start1..=end1 { set.insert(p - 1); }
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_and_ranges() {
        let r = parse_spec("1-3,5,10-").unwrap();
        assert_eq!(r[0], PageRange{ start:1, end: Some(3)});
        assert_eq!(r[1], PageRange{ start:5, end: Some(5)});
        assert_eq!(r[2], PageRange{ start:10, end: None});
    }

    #[test]
    fn expand_clamped_and_sorted() {
        let r = vec![PageRange{start:2, end:Some(4)}, PageRange{start:4, end:Some(6)}];
        let idx = expand_to_indexes(&r, 5);
        // pages: 2,3,4,5 (1-based) => 1,2,3,4 (0-based)
        assert_eq!(idx, vec![1,2,3,4]);
    }

    #[test]
    fn support_open_start_and_end() {
        let r = parse_spec("-2,4-",).unwrap();
        let idx = expand_to_indexes(&r, 5);
        // -2 => 1..=2 => 0,1 ; 4- => 4..=5 => 3,4
        assert_eq!(idx, vec![0,1,3,4]);
    }
}
