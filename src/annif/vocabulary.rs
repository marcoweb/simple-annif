#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vocabulary {
    pub id: String,
    pub languages: Vec<String>,
    pub size: Option<usize>,
    pub loaded: bool,
}
