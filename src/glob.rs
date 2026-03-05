use color_eyre::eyre::{eyre, ContextCompat};
use color_eyre::Result;
use regex_lite::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub struct Glob {
    with_protocol: Regex,
    without_protocol: Regex,
}

impl Glob {
    pub fn new(glob: &str) -> Result<Self> {
        build_glob(glob)
    }

    pub fn is_match(&self, url: &str) -> bool {
        let protocol_index = url.find(PROTOCOL_SEPARATOR);
        let regex = match protocol_index {
            Some(_) => &self.with_protocol,
            None => &self.without_protocol,
        };
        regex.is_match(url)
    }
}

impl<'de> Deserialize<'de> for Glob {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Glob, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(D::Error::custom)
    }
}

const MATCH_ONE_SEGMENT: &str = r"[^\.:/]*?";
const MATCH_ANYTHING: &str = ".*?";
const PROTOCOL_SEPARATOR: &str = "://";

fn build_glob(glob: &str) -> Result<Glob> {
    let protocol_index = glob.find(PROTOCOL_SEPARATOR)
        .with_context(|| eyre!("Invalid glob '{glob}', missing protocol separator '://'"))?;
    let glob_without_protocol = &glob[(protocol_index + PROTOCOL_SEPARATOR.len())..];

    let with_protocol = glob_to_regex(glob, protocol_index)?;
    let without_protocol = glob_to_regex(glob_without_protocol, 0)?;

    Ok(Glob {
        with_protocol,
        without_protocol,
    })
}

fn glob_to_regex(glob: &str, protocol_index: usize) -> Result<Regex> {
    let url_query_params_index = glob.chars().skip(protocol_index + 1)
        .position(|c| c == '?')
        .map(|it| it + protocol_index + 1);

    let mut regex_pattern = String::with_capacity(glob.len() * 2);
    regex_pattern.push_str("(?i)^");
    let mut index = 0;

    while index < glob.len() {
        let current = glob.chars().nth(index).unwrap();
        let next = glob.chars().nth(index + 1);

        match (current, next) {
            ('/', _) if (url_query_params_index.is_none() && index > protocol_index + 2)
                || Some(index + 1) == url_query_params_index => {
                regex_pattern.push_str("/?");
            }
            ('*', Some('*')) => {
                let pattern = if index < protocol_index {
                    MATCH_ONE_SEGMENT // We are in the start of the URL, match only the protocol
                } else {
                    MATCH_ANYTHING
                };
                regex_pattern.push_str(pattern);
                index += 1;
            },
            ('*', _) => {
                let pattern = if url_query_params_index.filter(|&it| index > it).is_some() {
                    MATCH_ANYTHING // We are in the query params, match everything until the end
                } else {
                    MATCH_ONE_SEGMENT
                };
                regex_pattern.push_str(pattern)
            },
            _ => {
                if is_regex_meta_character(current) {
                    regex_pattern.push('\\');
                }
                regex_pattern.push(current);
            }
        }
        index += 1;
    }
    if url_query_params_index.is_none() && !regex_pattern.ends_with("/?") {
        regex_pattern.push_str("/?");
    }
    regex_pattern.push('$');

    Ok(Regex::new(&regex_pattern)?)
}

fn is_regex_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{'
        | '}' | '^' | '$' | '#' | '&' | '-' | '~' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matches(glob: &str, url: &str) {
        let g = Glob::new(glob).unwrap_or_else(|e| panic!("Failed to create glob '{glob}': {e}"));
        assert!(g.is_match(url), "Expected glob '{glob}' to match URL '{url}'");
    }

    fn assert_no_match(glob: &str, url: &str) {
        let g = Glob::new(glob).unwrap_or_else(|e| panic!("Failed to create glob '{glob}': {e}"));
        assert!(!g.is_match(url), "Expected glob '{glob}' NOT to match URL '{url}'");
    }

    fn regex_str(glob: &str) -> String {
        let protocol_index = glob.find(PROTOCOL_SEPARATOR).unwrap();
        glob_to_regex(glob, protocol_index).unwrap().as_str().to_string()
    }

    //// Literal URL matching

    #[test]
    fn literal_exact_match() {
        assert_matches("https://example.com", "https://example.com");
    }

    #[test]
    fn literal_different_domain_no_match() {
        assert_no_match("https://example.com", "https://other.com");
    }

    #[test]
    fn literal_different_protocol_no_match() {
        assert_no_match("https://example.com", "http://example.com");
    }

    #[test]
    fn literal_no_partial_prefix_match() {
        assert_no_match("https://example.com", "https://example.com.evil.com");
    }

    #[test]
    fn literal_no_partial_suffix_match() {
        assert_no_match("https://example.com/path", "https://example.com/pathextra");
    }

    /// Single * wildcard

    #[test]
    fn single_star_matches_subdomain_segment() {
        assert_matches("https://*.example.com", "https://www.example.com");
    }

    #[test]
    fn single_star_does_not_cross_dot() {
        assert_no_match("https://*.example.com", "https://a.b.example.com");
    }

    #[test]
    fn single_star_does_not_cross_slash() {
        assert_no_match("https://example.com/*", "https://example.com/a/b");
    }

    #[test]
    fn single_star_matches_empty() {
        assert_matches("https://*example.com", "https://example.com");
    }

    #[test]
    fn single_star_in_path() {
        assert_matches("https://example.com/*/page", "https://example.com/foo/page");
    }

    #[test]
    fn single_star_path_no_cross_dot() {
        assert_no_match("https://example.com/*", "https://example.com/foo.bar");
    }

    #[test]
    fn single_star_does_not_cross_colon() {
        assert_no_match("https://*.example.com", "https://a:b.example.com");
    }

    /// Double ** wildcard

    #[test]
    fn double_star_crosses_segments() {
        assert_matches("https://example.com/**", "https://example.com/a/b/c");
    }

    #[test]
    fn double_star_crosses_dots() {
        assert_matches("https://**example.com", "https://a.b.c.example.com");
    }

    #[test]
    fn double_star_matches_empty() {
        assert_matches("https://**example.com", "https://example.com");
    }

    #[test]
    fn double_star_deep_path() {
        assert_matches("https://example.com/**/page", "https://example.com/a/b/c/page");
    }

    #[test]
    fn double_star_entire_domain() {
        assert_matches("https://**", "https://anything.goes.here/and/paths");
    }

    #[test]
    fn double_star_in_middle_of_domain() {
        assert_matches("https://www.**", "https://www.example.com/path");
    }

    #[test]
    fn double_star_subdomain_deep() {
        assert_matches("https://**/*.com", "https://sub.domain.example.com");
    }

    /// Protocol wildcard

    #[test]
    fn double_star_protocol_matches_https() {
        assert_matches("**://example.com", "https://example.com");
    }

    #[test]
    fn double_star_protocol_restricted_to_segment() {
        // ** before :// uses MATCH_ONE_SEGMENT, so it shouldn't cross dots/slashes
        assert_no_match("**://example.com", "https.extra://example.com");
    }

    #[test]
    fn partial_protocol_wildcard() {
        assert_matches("http*://example.com", "https://example.com");
    }

    #[test]
    fn partial_protocol_no_cross_segment() {
        assert_no_match("http*://example.com", "http.x://example.com");
    }

    /// Query parameters

    #[test]
    fn query_literal_match() {
        assert_matches("https://example.com/search?q=test", "https://example.com/search?q=test");
    }

    #[test]
    fn query_literal_no_match() {
        assert_no_match("https://example.com/search?q=test", "https://example.com/search?q=other");
    }

    #[test]
    fn query_star_matches_value() {
        assert_matches("https://example.com/search?q=*", "https://example.com/search?q=anything");
    }

    #[test]
    fn query_star_matches_across_separators() {
        // * in query params uses MATCH_ANYTHING
        assert_matches("https://example.com/search?q=*", "https://example.com/search?q=a&b=c");
    }

    #[test]
    fn query_slash_before_question_optional() {
        assert_matches("https://example.com/search?q=test", "https://example.com/search?q=test");
    }

    #[test]
    fn query_path_slashes_stay_literal() {
        // Slashes before the query (not immediately before ?) should be literal
        assert_no_match("https://example.com/a/b?q=1", "https://example.com/ab?q=1");
    }

    #[test]
    fn query_without_slash_before_question() {
        assert_matches("https://example.com/path/?q=1", "https://example.com/path?q=1");
    }

    /// Trailing slash

    #[test]
    fn trailing_slash_optional_when_absent() {
        assert_matches("https://example.com", "https://example.com/");
    }

    #[test]
    fn trailing_slash_optional_when_present() {
        assert_matches("https://example.com/", "https://example.com");
    }

    #[test]
    fn trailing_slash_on_path() {
        assert_matches("https://example.com/path", "https://example.com/path/");
    }

    #[test]
    fn trailing_slash_not_optional_with_query() {
        // When query params are present, trailing slash behavior doesn't apply to end
        assert_matches("https://example.com/path?q=1", "https://example.com/path?q=1");
    }

    /// Case insensitivity

    #[test]
    fn case_insensitive_domain() {
        assert_matches("https://EXAMPLE.COM", "https://example.com");
    }

    #[test]
    fn case_insensitive_protocol() {
        assert_matches("HTTPS://example.com", "https://example.com");
    }

    #[test]
    fn case_insensitive_path() {
        assert_matches("https://example.com/PATH", "https://example.com/path");
    }

    #[test]
    fn case_insensitive_with_wildcards() {
        assert_matches("https://*.EXAMPLE.COM", "https://www.example.com");
    }

    /// Metacharacter escaping

    #[test]
    fn dot_is_escaped() {
        assert_no_match("https://example.com", "https://exampleXcom");
    }

    #[test]
    fn plus_is_escaped() {
        assert_matches("https://example.com/a+b", "https://example.com/a+b");
        assert_no_match("https://example.com/a+b", "https://example.com/aab");
    }

    #[test]
    fn parens_are_escaped() {
        assert_matches("https://example.com/(page)", "https://example.com/(page)");
    }

    #[test]
    fn brackets_are_escaped() {
        assert_matches("https://example.com/[1]{2}", "https://example.com/[1]{2}");
    }

    /// Error cases

    #[test]
    fn missing_protocol_separator_is_error() {
        assert!(Glob::new("example.com").is_err());
    }

    #[test]
    fn error_message_contains_glob() {
        let err = Glob::new("example.com").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("example.com"), "Error should contain the glob: {msg}");
        assert!(msg.contains("://"), "Error should mention '://': {msg}");
    }

    #[test]
    fn minimal_valid_glob() {
        assert!(Glob::new("a://b").is_ok());
    }

    /// Serde deserialization

    #[test]
    fn serde_valid_glob() {
        let g: Glob = serde_json::from_str(r#""https://example.com""#).unwrap();
        assert!(g.is_match("https://example.com"));
    }

    #[test]
    fn serde_invalid_glob_is_error() {
        let result: core::result::Result<Glob, _> = serde_json::from_str(r#""no-protocol""#);
        assert!(result.is_err());
    }

    #[test]
    fn serde_inside_struct() {
        #[derive(Deserialize)]
        struct Config {
            pattern: Glob,
        }
        let c: Config = serde_json::from_str(r#"{"pattern": "https://*.example.com"}"#).unwrap();
        assert!(c.pattern.is_match("https://www.example.com"));
    }

    /// Without-protocol matching

    #[test]
    fn without_protocol_plain_domain() {
        assert_matches("https://example.com", "example.com");
    }

    #[test]
    fn without_protocol_with_path() {
        assert_matches("https://example.com/path", "example.com/path");
    }

    #[test]
    fn without_protocol_wildcard() {
        assert_matches("https://*.example.com", "www.example.com");
    }

    #[test]
    fn without_protocol_no_match() {
        assert_no_match("https://example.com", "other.com");
    }

    /// Edge cases

    #[test]
    fn port_number() {
        assert_matches("https://localhost:8080", "https://localhost:8080");
    }

    #[test]
    fn different_port_number() {
        assert_no_match("https://localhost:8080", "https://localhost:8081");
    }

    #[test]
    fn multiple_wildcards() {
        assert_matches("https://*.*.com/*", "https://sub.example.com/page");
    }

    #[test]
    fn triple_star_treated_as_double_plus_single() {
        // *** = ** consumed first (MATCH_ANYTHING), then * (MATCH_ONE_SEGMENT)
        assert_matches("https://***example.com", "https://a.b.example.com");
    }

    #[test]
    fn empty_path_after_domain() {
        assert_matches("https://example.com", "https://example.com");
    }

    /// Bug regressions

    #[test]
    fn no_double_dollar_in_regex() {
        let r = regex_str("https://example.com");
        assert!(!r.contains("$$"), "Regex should not contain '$$': {r}");
    }

    #[test]
    fn protocol_slashes_are_mandatory() {
        assert_no_match("https://example.com", "https:example.com");
    }

    #[test]
    fn protocol_single_slash_no_match() {
        assert_no_match("https://example.com", "https:/example.com");
    }

    #[test]
    fn trailing_slash_still_optional_after_fix() {
        assert_matches("https://example.com/path", "https://example.com/path/");
        assert_matches("https://example.com/path/", "https://example.com/path");
    }

    #[test]
    fn path_slashes_still_optional() {
        // Internal path slashes should remain optional (the fix only protects protocol slashes)
        assert_matches("https://example.com/a/b", "https://example.com/ab");
    }

    /// Real-world patterns

    #[test]
    fn google_search() {
        assert_matches(
            "https://www.google.com/search?q=*",
            "https://www.google.com/search?q=rust+programming",
        );
    }

    #[test]
    fn specific_subdomain_pattern() {
        assert_matches("https://docs.**.com/**", "https://docs.example.com/en/latest/guide");
    }

    #[test]
    fn tracking_url_pattern() {
        assert_matches(
            "https://**.tracking.com/**",
            "https://pixel.tracking.com/collect?id=123&event=click",
        );
    }
}