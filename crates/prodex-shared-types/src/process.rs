#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
}
