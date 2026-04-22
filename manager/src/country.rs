#[derive(Clone, Copy, PartialEq)]
pub struct CountryOption {
    pub code: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
}

pub const COUNTRY_OPTIONS: &[CountryOption] = &[
    CountryOption { code: "AM", name: "Armenia", aliases: &[] },
    CountryOption { code: "AU", name: "Australia", aliases: &[] },
    CountryOption { code: "AT", name: "Austria", aliases: &[] },
    CountryOption { code: "BE", name: "Belgium", aliases: &[] },
    CountryOption { code: "BR", name: "Brazil", aliases: &[] },
    CountryOption { code: "BG", name: "Bulgaria", aliases: &[] },
    CountryOption { code: "CA", name: "Canada", aliases: &[] },
    CountryOption { code: "CN", name: "China", aliases: &[] },
    CountryOption { code: "HR", name: "Croatia", aliases: &[] },
    CountryOption { code: "CY", name: "Cyprus", aliases: &[] },
    CountryOption { code: "CZ", name: "Czech Republic", aliases: &["Czechia"] },
    CountryOption { code: "DK", name: "Denmark", aliases: &[] },
    CountryOption { code: "EE", name: "Estonia", aliases: &[] },
    CountryOption { code: "FI", name: "Finland", aliases: &[] },
    CountryOption { code: "FR", name: "France", aliases: &[] },
    CountryOption { code: "GE", name: "Georgia", aliases: &[] },
    CountryOption { code: "DE", name: "Germany", aliases: &[] },
    CountryOption { code: "GR", name: "Greece", aliases: &[] },
    CountryOption { code: "HK", name: "Hong Kong", aliases: &[] },
    CountryOption { code: "HU", name: "Hungary", aliases: &[] },
    CountryOption { code: "IS", name: "Iceland", aliases: &[] },
    CountryOption { code: "IN", name: "India", aliases: &[] },
    CountryOption { code: "ID", name: "Indonesia", aliases: &[] },
    CountryOption { code: "IE", name: "Ireland", aliases: &[] },
    CountryOption { code: "IL", name: "Israel", aliases: &[] },
    CountryOption { code: "IT", name: "Italy", aliases: &[] },
    CountryOption { code: "JP", name: "Japan", aliases: &[] },
    CountryOption { code: "KZ", name: "Kazakhstan", aliases: &[] },
    CountryOption { code: "LV", name: "Latvia", aliases: &[] },
    CountryOption { code: "LT", name: "Lithuania", aliases: &[] },
    CountryOption { code: "LU", name: "Luxembourg", aliases: &[] },
    CountryOption { code: "MY", name: "Malaysia", aliases: &[] },
    CountryOption { code: "MD", name: "Moldova", aliases: &[] },
    CountryOption { code: "NL", name: "Netherlands", aliases: &["Holland"] },
    CountryOption { code: "NZ", name: "New Zealand", aliases: &[] },
    CountryOption { code: "NO", name: "Norway", aliases: &[] },
    CountryOption { code: "PL", name: "Poland", aliases: &[] },
    CountryOption { code: "PT", name: "Portugal", aliases: &[] },
    CountryOption { code: "RO", name: "Romania", aliases: &[] },
    CountryOption { code: "RU", name: "Russia", aliases: &[] },
    CountryOption { code: "RS", name: "Serbia", aliases: &[] },
    CountryOption { code: "SG", name: "Singapore", aliases: &[] },
    CountryOption { code: "SK", name: "Slovakia", aliases: &[] },
    CountryOption { code: "SI", name: "Slovenia", aliases: &[] },
    CountryOption {
        code: "KR",
        name: "South Korea",
        aliases: &["Korea", "Republic of Korea"],
    },
    CountryOption { code: "ES", name: "Spain", aliases: &[] },
    CountryOption { code: "SE", name: "Sweden", aliases: &[] },
    CountryOption { code: "CH", name: "Switzerland", aliases: &[] },
    CountryOption { code: "TH", name: "Thailand", aliases: &[] },
    CountryOption { code: "TR", name: "Turkey", aliases: &["Turkiye"] },
    CountryOption { code: "UA", name: "Ukraine", aliases: &[] },
    CountryOption {
        code: "AE",
        name: "United Arab Emirates",
        aliases: &["UAE"],
    },
    CountryOption {
        code: "GB",
        name: "United Kingdom",
        aliases: &["UK", "Britain", "Great Britain"],
    },
    CountryOption {
        code: "US",
        name: "United States",
        aliases: &["USA", "United States of America", "America"],
    },
    CountryOption { code: "VN", name: "Vietnam", aliases: &[] },
];

pub fn normalize_country_code(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

pub fn flag_emoji(country: &str) -> String {
    let normalized = normalize_country_code(country);
    if normalized.len() != 2 {
        return String::new();
    }

    normalized
        .chars()
        .map(|ch| char::from_u32(0x1F1E6 + (ch as u32 - 'A' as u32)).unwrap_or(ch))
        .collect()
}

pub fn find_country_by_query(query: &str) -> Option<CountryOption> {
    let normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    COUNTRY_OPTIONS.iter().copied().find(|country| {
        country.code.eq_ignore_ascii_case(query.trim())
            || country.name.eq_ignore_ascii_case(query.trim())
            || country
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(query.trim()))
    })
}

pub fn search_countries(query: &str) -> Vec<CountryOption> {
    let normalized = query.trim().to_lowercase();
    let mut results: Vec<CountryOption> = COUNTRY_OPTIONS
        .iter()
        .copied()
        .filter(|country| {
            if normalized.is_empty() {
                return true;
            }

            let code = country.code.to_lowercase();
            let name = country.name.to_lowercase();
            code.starts_with(&normalized)
                || name.starts_with(&normalized)
                || name.contains(&normalized)
                || country.aliases.iter().any(|alias| {
                    let alias = alias.to_lowercase();
                    alias.starts_with(&normalized) || alias.contains(&normalized)
                })
        })
        .collect();
    results.truncate(8);
    results
}

pub fn country_display(country: CountryOption) -> String {
    let flag = flag_emoji(country.code);
    format!("{} {} ({})", flag, country.name, country.code)
}

pub fn country_name(country: &str) -> Option<&'static str> {
    find_country_by_query(country).map(|country| country.name)
}
