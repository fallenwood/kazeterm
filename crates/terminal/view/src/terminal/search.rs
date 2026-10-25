/// Active search parameters stored in Terminal so the search can be
/// automatically re-executed whenever terminal content changes.
pub struct SearchState {
  pub query: String,
  pub match_case: bool,
  pub match_whole: bool,
  pub use_regex: bool,
  /// Pre-compiled regex (only set when `use_regex` is true and the pattern is valid).
  pub(crate) compiled_regex: Option<regex::Regex>,
}

impl SearchState {
  pub fn new(query: String, match_case: bool, match_whole: bool, use_regex: bool) -> Option<Self> {
    if query.is_empty() {
      return None;
    }

    let compiled_regex = if use_regex {
      let pattern = if match_whole {
        format!(r"\b{}\b", query)
      } else {
        query.clone()
      };
      let result = if match_case {
        regex::Regex::new(&pattern)
      } else {
        regex::Regex::new(&format!("(?i){}", pattern))
      };
      match result {
        Ok(re) => Some(re),
        Err(_) => return None, // invalid regex
      }
    } else {
      None
    };

    Some(Self {
      query,
      match_case,
      match_whole,
      use_regex,
      compiled_regex,
    })
  }
}
