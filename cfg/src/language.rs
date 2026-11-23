use strum::VariantArray;

#[derive(VariantArray)]
pub enum Language {
    En,
    De,
}

impl Language {
    pub fn as_locale(&self) -> &str {
        match *self {
            Self::En => "en_US.UTF-8",
            Self::De => "de_DE.UTF-8",
        }
    }
}
