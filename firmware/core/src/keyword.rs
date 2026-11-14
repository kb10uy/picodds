pub(crate) fn parse_keyword<T: Copy>(text: &[u8], keywords: &[(&str, T)]) -> Option<T> {
    keywords
        .iter()
        .find(|(keyword, _)| text.eq_ignore_ascii_case(keyword.as_bytes()))
        .map(|&(_, value)| value)
}

pub(crate) fn keyword<T: PartialEq>(value: T, keywords: &[(&'static str, T)]) -> &'static str {
    keywords
        .iter()
        .find(|(_, candidate)| *candidate == value)
        .map_or("", |&(keyword, _)| keyword)
}
