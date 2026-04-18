use regex::Regex;
use std::{
  ops::Range,
  time::{Duration, Instant},
};
use terminal_kernel::{
  TerminalBackend,
  index::{Boundary, Column, Direction as AlacDirection, Point as AlacPoint},
  term::cell::Flags,
};
use url::Url;

const URL_REGEX: &str = r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`']+"#;
const WIDE_CHAR_SPACERS: Flags =
  Flags::from_bits(Flags::LEADING_WIDE_CHAR_SPACER.bits() | Flags::WIDE_CHAR_SPACER.bits())
    .unwrap();

pub struct RegexSearches {
  path_hyperlink_regexes: Vec<Regex>,
  path_hyperlink_timeout: Duration,
}

impl Default for RegexSearches {
  fn default() -> Self {
    Self {
      path_hyperlink_regexes: Vec::default(),
      path_hyperlink_timeout: Duration::default(),
    }
  }
}

pub(super) fn find_from_grid_point(
  backend: &dyn TerminalBackend,
  point: AlacPoint,
  regex_searches: &mut RegexSearches,
) -> Option<(String, bool, std::ops::RangeInclusive<AlacPoint>)> {
  // Delegate URL/OSC8 detection to the backend.
  if let Some(result) = backend.find_hyperlink_at(point, URL_REGEX) {
    let (url, is_url, range) = result;
    // OSC8 hyperlinks (explicit, set by the application) are returned as-is.
    // Regex-matched URLs need trailing punctuation sanitization.
    let is_osc8 = backend.cell_at(point).hyperlink().is_some();
    if is_url && !is_osc8 {
      let (sanitized_url, sanitized_match) = sanitize_url_punctuation(url, range, backend);
      // Apply file:// path handling.
      let (final_url, final_is_url, final_match) =
        handle_file_url(sanitized_url, true, sanitized_match);
      return Some((final_url, final_is_url, final_match));
    }
    // OSC8: apply file:// handling but no sanitization.
    let (final_url, final_is_url, final_match) = handle_file_url(url, is_url, range);
    return Some((final_url, final_is_url, final_match));
  }

  // Fall back to path matching.
  let (line_start, line_end) = (
    backend.line_search_left(point),
    backend.line_search_right(point),
  );
  path_match(
    backend,
    line_start,
    line_end,
    point,
    &mut regex_searches.path_hyperlink_regexes,
    regex_searches.path_hyperlink_timeout,
  )
  .map(|(path, path_match)| (path, false, path_match))
}

/// Convert `file://` URLs to local file paths; pass other URLs through unchanged.
fn handle_file_url(
  maybe_url_or_path: String,
  is_url: bool,
  word_match: std::ops::RangeInclusive<AlacPoint>,
) -> (String, bool, std::ops::RangeInclusive<AlacPoint>) {
  if is_url && maybe_url_or_path.starts_with("file://") {
    if let Ok(url) = Url::parse(&maybe_url_or_path)
      && let Ok(path) = url.to_file_path()
    {
      return (path.to_string_lossy().into_owned(), false, word_match);
    }
    let path = maybe_url_or_path
      .strip_prefix("file://")
      .unwrap_or(&maybe_url_or_path);
    return (path.to_string(), false, word_match);
  }
  (maybe_url_or_path, is_url, word_match)
}

fn sanitize_url_punctuation(
  url: String,
  url_match: std::ops::RangeInclusive<AlacPoint>,
  backend: &dyn TerminalBackend,
) -> (String, std::ops::RangeInclusive<AlacPoint>) {
  let mut sanitized_url = url;
  let mut chars_trimmed = 0;

  // Count parentheses in the URL
  let (open_parens, mut close_parens) =
    sanitized_url
      .chars()
      .fold((0, 0), |(opens, closes), c| match c {
        '(' => (opens + 1, closes),
        ')' => (opens, closes + 1),
        _ => (opens, closes),
      });

  // Remove trailing characters that shouldn't be at the end of URLs
  while let Some(last_char) = sanitized_url.chars().last() {
    let should_remove = match last_char {
      // These may be part of a URL but not at the end. It's not that the spec
      // doesn't allow them, but they are frequently used in plain text as delimiters
      // where they're not meant to be part of the URL.
      '.' | ',' | ':' | ';' => true,
      '(' => true,
      ')' if close_parens > open_parens => {
        close_parens -= 1;

        true
      }
      _ => false,
    };

    if should_remove {
      sanitized_url.pop();
      chars_trimmed += 1;
    } else {
      break;
    }
  }

  if chars_trimmed > 0 {
    let new_end = backend.point_sub(*url_match.end(), Boundary::Grid, chars_trimmed);
    let sanitized_match = *url_match.start()..=new_end;
    (sanitized_url, sanitized_match)
  } else {
    (sanitized_url, url_match)
  }
}

fn path_match(
  backend: &dyn TerminalBackend,
  line_start: AlacPoint,
  line_end: AlacPoint,
  hovered: AlacPoint,
  path_hyperlink_regexes: &mut Vec<Regex>,
  path_hyperlink_timeout: Duration,
) -> Option<(String, std::ops::RangeInclusive<AlacPoint>)> {
  if path_hyperlink_regexes.is_empty() || path_hyperlink_timeout.as_millis() == 0 {
    return None;
  }
  debug_assert!(line_start <= hovered);
  debug_assert!(line_end >= hovered);
  let search_start_time = Instant::now();

  let timed_out = || {
    let elapsed_time = Instant::now().saturating_duration_since(search_start_time);
    (elapsed_time > path_hyperlink_timeout)
      .then_some((elapsed_time.as_millis(), path_hyperlink_timeout.as_millis()))
  };

  // Build cell-accurate string from the grid line. bounds_to_string compresses
  // tabs into single spaces, so we iterate cells directly instead.
  let mut line =
    String::with_capacity((line_end.line.0 - line_start.line.0 + 1) as usize * backend.columns());
  let first_cell = backend.cell_at(line_start);
  let mut prev_len = 0;
  line.push(first_cell.c);
  let mut prev_char_is_space = first_cell.c == ' ';
  let mut hovered_point_byte_offset = None;
  let mut hovered_word_start_offset = None;
  let mut hovered_word_end_offset = None;

  if line_start == hovered {
    hovered_point_byte_offset = Some(0);
    if first_cell.c != ' ' {
      hovered_word_start_offset = Some(0);
    }
  }

  backend.iter_from(line_start, &mut |cell_point, cell| {
    if cell_point > line_end {
      return false;
    }

    if !cell.flags.intersects(WIDE_CHAR_SPACERS) {
      prev_len = line.len();
      match cell.c {
        ' ' | '\t' => {
          if hovered_point_byte_offset.is_some()
            && !prev_char_is_space
            && hovered_word_end_offset.is_none()
          {
            hovered_word_end_offset = Some(line.len());
          }
          line.push(' ');
          prev_char_is_space = true;
        }
        c => {
          if hovered_point_byte_offset.is_none() && prev_char_is_space {
            hovered_word_start_offset = Some(line.len());
          }
          line.push(c);
          prev_char_is_space = false;
        }
      }
    }

    if cell_point == hovered {
      debug_assert!(hovered_point_byte_offset.is_none());
      hovered_point_byte_offset = Some(prev_len);
    }

    true
  });

  let line = line.trim_ascii_end();
  let hovered_point_byte_offset = hovered_point_byte_offset?;
  let hovered_word_range = {
    let word_start_offset = hovered_word_start_offset.unwrap_or(0);
    (word_start_offset != 0)
      .then_some(word_start_offset..hovered_word_end_offset.unwrap_or(line.len()))
  };
  if line.len() <= hovered_point_byte_offset {
    return None;
  }
  let found_from_range =
    |path_range: Range<usize>, link_range: Range<usize>, position: Option<(u32, Option<u32>)>| {
      let advance_point_by_str = |mut point: AlacPoint, s: &str| {
        for _ in s.chars() {
          point = backend.expand_wide(point, AlacDirection::Right);
          point = backend.point_add(point, Boundary::Grid, 1);
        }

        let flags = backend.cell_at(point).flags;
        if flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
          AlacPoint::new(point.line + 1, Column(0))
        } else if flags.contains(Flags::WIDE_CHAR_SPACER) {
          AlacPoint::new(point.line, point.column - 1)
        } else {
          point
        }
      };

      let link_start = advance_point_by_str(line_start, &line[..link_range.start]);
      let link_end = advance_point_by_str(link_start, &line[link_range]);
      let link_end_expanded = backend.expand_wide(link_end, AlacDirection::Left);
      let link_end_final = backend.point_sub(link_end_expanded, Boundary::Grid, 1);
      let link_match = link_start..=link_end_final;

      (
        {
          let mut path = line[path_range].to_string();
          position.inspect(|(line, column)| {
            path += &format!(":{line}");
            column.inspect(|column| path += &format!(":{column}"));
          });
          path
        },
        link_match,
      )
    };

  for regex in path_hyperlink_regexes {
    let mut path_found = false;

    for (line_start_offset, captures) in std::iter::once(
      regex
        .captures_iter(line)
        .next()
        .map(|captures| (0, captures)),
    )
    .chain(std::iter::once_with(|| {
      if let Some(hovered_word_range) = &hovered_word_range {
        regex
          .captures_iter(&line[hovered_word_range.clone()])
          .next()
          .map(|captures| (hovered_word_range.start, captures))
      } else {
        None
      }
    }))
    .flatten()
    {
      path_found = true;
      let match_range = captures.get(0).unwrap().range();
      let (mut path_range, line_column) = if let Some(path) = captures.name("path") {
        let parse = |name: &str| {
          captures
            .name(name)
            .and_then(|capture| capture.as_str().parse().ok())
        };

        (
          path.range(),
          parse("line").map(|line| (line, parse("column"))),
        )
      } else {
        (match_range.clone(), None)
      };
      let mut link_range = captures
        .name("link")
        .map_or_else(|| match_range.clone(), |link| link.range());

      path_range.start += line_start_offset;
      path_range.end += line_start_offset;
      link_range.start += line_start_offset;
      link_range.end += line_start_offset;

      if !link_range.contains(&hovered_point_byte_offset) {
        // No match, just skip.
        continue;
      }
      let found = found_from_range(path_range, link_range, line_column);

      if found.1.contains(&hovered) {
        return Some(found);
      }
    }

    if path_found {
      return None;
    }

    if let Some((_timed_out_ms, _timeout_ms)) = timed_out() {
      return None;
    }
  }

  None
}
