pub trait StrExt {
    fn find_after(&self, pattern: &str) -> Option<usize>;
    fn find_token_after(&self, pattern: &str) -> Option<std::ops::Range<usize>>;
}

impl StrExt for str {
    fn find_after(&self, pattern: &str) -> Option<usize> {
        self.find(pattern).map(|pos| pos + pattern.len())
    }

    fn find_token_after(&self, pattern: &str) -> Option<std::ops::Range<usize>> {
        let pos = self.find_after(pattern)?;
        let ws = self[pos..].find(|c: char| c.is_whitespace())?;
        Some(pos..pos + ws)
    }
}
