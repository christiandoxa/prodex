use clap::ValueEnum;

#[derive(
    Clone, Copy, Debug, ValueEnum, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum PresidioLanguageMode {
    #[default]
    Fixed,
    Auto,
    Multi,
}

impl PresidioLanguageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Auto => "auto",
            Self::Multi => "multi",
        }
    }
}
