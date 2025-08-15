use crate::Unit;

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Percent => "%",
            Self::Px => "px",
            Self::Em => "em",
            Self::Ex => "ex",
            Self::In => "in",
            Self::Cm => "cm",
            Self::Mm => "mm",
            Self::Pt => "pt",
            Self::Pc => "pc",
            #[cfg(feature = "v2_59")]
            Self::Ch => "ch",
            Self::__Unknown(_) => "unknown",
        };

        f.write_str(s)
    }
}
