#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct DocSection {
    pub title: String,
    pub level: usize,
    pub file_name: String,
    pub file_path: String,
    pub content: String,
    pub line_number: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct DocDocument {
    pub title: String,
    pub file_name: String,
    pub file_path: String,
    pub sections: Vec<DocSection>,
}

#[derive(Debug, Clone, Default)]
pub struct DocIndex {
    pub documents: Vec<DocDocument>,
    pub sections: Vec<DocSection>,
}
