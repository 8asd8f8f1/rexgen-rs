#[derive(Debug, Clone)]
pub(crate) struct Limits {
    pub min_len: usize,
    pub max_len: Option<usize>,
}
