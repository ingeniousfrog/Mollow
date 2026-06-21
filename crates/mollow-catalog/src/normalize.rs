/// Normalizes a hardware model string for catalog lookup.
#[must_use]
pub fn normalize_model(input: &str) -> String {
    let mut value = input.to_ascii_lowercase();
    for token in [
        "(r)",
        "(tm)",
        "intel(r)",
        "core(tm)",
        "nvidia",
        "geforce",
        "radeon",
        "processor",
        "cpu",
        "@",
    ] {
        value = value.replace(token, " ");
    }
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns true when every token in `pattern` appears in order within `normalized`.
#[must_use]
pub fn matches_pattern(normalized: &str, pattern: &str) -> bool {
    let pattern = normalize_model(pattern);
    if pattern.is_empty() {
        return false;
    }
    if normalized.contains(&pattern) {
        return true;
    }
    pattern
        .split_whitespace()
        .all(|token| normalized.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_vendor_noise() {
        assert_eq!(
            normalize_model("Intel(R) Core(TM) i7-12700K CPU @ 3.60GHz"),
            "intel core i7-12700k 3.60ghz"
        );
    }

    #[test]
    fn matches_pattern_accepts_token_subset() {
        let normalized = normalize_model("12th Gen Intel Core i7-12700K");
        assert!(matches_pattern(&normalized, "core i7-12700k"));
    }
}
