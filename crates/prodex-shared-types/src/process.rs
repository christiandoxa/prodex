#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: u32,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProdexProcessInfo {
    pub pid: u32,
    pub command: String,
    pub runtime: bool,
}
