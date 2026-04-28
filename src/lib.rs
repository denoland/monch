// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;

/// An error that occurred while parsing.
#[derive(Debug)]
pub enum ParseError<'a> {
  Backtrace,
  /// Parsing should completely fail.
  Failure(Box<ParseErrorFailure<'a>>),
}

/// A complete parsing failure along with the location
/// the error occurred and the error message.
#[derive(Debug, Clone)]
pub struct ParseErrorFailure<'a> {
  pub input: &'a str,
  pub message: Cow<'static, str>,
}

impl<'a> ParseErrorFailure<'a> {
  #[inline]
  pub fn new(input: &'a str, message: impl Into<Cow<'static, str>>) -> Self {
    ParseErrorFailure {
      input,
      message: message.into(),
    }
  }

  /// Opinionated helper used to fail for trailing input.
  #[inline]
  pub fn new_for_trailing_input(input: &'a str) -> Self {
    ParseErrorFailure::new(input, "Unexpected character.")
  }

  /// Opinionated helper to turn this failure into a result.
  pub fn into_result<T>(self) -> Result<T, ParseErrorFailureError> {
    Err(self.into_error())
  }

  /// Opinionated helper to turn this failure into a `ParseErrorFailureError`.
  pub fn into_error(self) -> ParseErrorFailureError {
    // truncate the output to prevent wrapping in the console. find the byte
    // boundary at the 60th char so the snippet can be sliced and allocated
    // in a single `to_owned` rather than char-by-char into a String.
    let snippet = match self.input.char_indices().nth(60) {
      Some((idx, _)) => self.input[..idx].to_owned(),
      None => self.input.to_owned(),
    };
    ParseErrorFailureError {
      message: self.message,
      code_snippet: Some(snippet),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParseErrorFailureError {
  pub message: Cow<'static, str>,
  pub code_snippet: Option<String>,
}

impl ParseErrorFailureError {
  pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
    ParseErrorFailureError {
      message: message.into(),
      code_snippet: None,
    }
  }
}

impl std::fmt::Display for ParseErrorFailureError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(code_snippet) = &self.code_snippet {
      write!(f, "{}\n  {}\n  ~", self.message, code_snippet)
    } else {
      write!(f, "{}", self.message)
    }
  }
}

impl std::error::Error for ParseErrorFailureError {}

impl<'a> ParseError<'a> {
  #[inline]
  pub fn fail<O>(
    input: &'a str,
    message: impl Into<Cow<'static, str>>,
  ) -> ParseResult<'a, O> {
    Err(ParseError::Failure(Box::new(ParseErrorFailure::new(
      input, message,
    ))))
  }

  #[inline]
  pub fn backtrace<O>() -> ParseResult<'a, O> {
    Err(ParseError::Backtrace)
  }
}

pub type ParseResult<'a, O> = Result<(&'a str, O), ParseError<'a>>;

/// Opinionated helper that converts a combinator into a Result<T, String>
pub fn with_failure_handling<'a, T>(
  combinator: impl Fn(&'a str) -> ParseResult<T>,
) -> impl Fn(&'a str) -> Result<T, ParseErrorFailureError> {
  move |input| match combinator(input) {
    Ok((input, result)) => {
      if !input.is_empty() {
        ParseErrorFailure::new_for_trailing_input(input).into_result()
      } else {
        Ok(result)
      }
    }
    Err(ParseError::Backtrace) => {
      ParseErrorFailure::new_for_trailing_input(input).into_result()
    }
    Err(ParseError::Failure(e)) => (*e).into_result(),
  }
}

/// Recognizes a character.
#[inline]
pub fn ch<'a>(c: char) -> impl Fn(&'a str) -> ParseResult<'a, char> {
  // ASCII fast path: branch is on a value captured by the closure, so once
  // `ch` is inlined at the call site the branch folds away.
  let is_ascii = c.is_ascii();
  let len = c.len_utf8();
  let ascii_byte = c as u32 as u8;
  move |input: &'a str| {
    if is_ascii {
      match input.as_bytes().first() {
        Some(&b) if b == ascii_byte => Ok((&input[1..], c)),
        _ => ParseError::backtrace(),
      }
    } else {
      match input.chars().next() {
        Some(found) if found == c => Ok((&input[len..], c)),
        _ => ParseError::backtrace(),
      }
    }
  }
}

/// Gets the next character.
#[inline]
#[allow(clippy::needless_lifetimes)]
pub fn next_char<'a>(input: &'a str) -> ParseResult<'a, char> {
  match input.chars().next() {
    Some(next_char) => Ok((&input[next_char.len_utf8()..], next_char)),
    _ => ParseError::backtrace(),
  }
}

/// Recognizes any character in the provided string.
#[inline]
pub fn one_of<'a>(
  value: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, char> {
  // precompute an ASCII bitmap so the per-char check is O(1) instead of
  // a substring search on every call. non-ASCII chars fall back to a Vec.
  let mut ascii_bitmap: u128 = 0;
  let mut non_ascii: Vec<char> = Vec::new();
  for c in value.chars() {
    if c.is_ascii() {
      ascii_bitmap |= 1u128 << (c as u8);
    } else {
      non_ascii.push(c);
    }
  }
  move |input| {
    let (input, c) = next_char(input)?;
    let matched = if c.is_ascii() {
      (ascii_bitmap >> (c as u8)) & 1 != 0
    } else {
      non_ascii.contains(&c)
    };
    if matched {
      Ok((input, c))
    } else {
      ParseError::backtrace()
    }
  }
}

/// Recognizes a string.
///
/// Use [`tag_owned`] when the value is built at runtime.
#[inline]
pub fn tag<'a>(
  value: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  move |input| match input.strip_prefix(value) {
    Some(rest) => Ok((rest, &input[..value.len()])),
    None => Err(ParseError::Backtrace),
  }
}

/// Recognizes a runtime-built string.
///
/// Prefer [`tag`] when the value is a `&'static str`; this version owns its
/// tag, which is needed only when the string is computed at runtime.
#[inline]
pub fn tag_owned<'a>(
  value: String,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  move |input| match input.strip_prefix(value.as_str()) {
    Some(rest) => Ok((rest, &input[..value.len()])),
    None => Err(ParseError::Backtrace),
  }
}

/// Gets the substring found for the duration of the combinator.
#[inline]
pub fn substring<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  move |input| {
    let original_input = input;
    let (input, _) = combinator(input)?;
    let length = original_input.len() - input.len();
    Ok((input, &original_input[..length]))
  }
}

/// Skip the input while the condition is true.
#[inline]
pub fn skip_while<'a>(
  cond: impl Fn(char) -> bool,
) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
  move |input| {
    for (pos, c) in input.char_indices() {
      if !cond(c) {
        return Ok((&input[pos..], ()));
      }
    }
    // reached the end
    Ok(("", ()))
  }
}

/// Skip the input while the condition is true on each byte. Faster than
/// [`skip_while`] when the predicate is ASCII-only — a byte loop avoids
/// UTF-8 decoding. The predicate must return `false` for any non-ASCII
/// continuation byte for input slicing to land on a char boundary; in
/// practice this is satisfied automatically by ASCII-classifier predicates
/// like `is_ascii_digit` / `is_ascii_alphanumeric` / `== b' '`.
#[inline]
pub fn skip_while_byte<'a>(
  cond: impl Fn(u8) -> bool,
) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
  move |input| {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() && cond(bytes[i]) {
      i += 1;
    }
    Ok((&input[i..], ()))
  }
}

/// Takes a substring while the condition is true.
#[inline]
pub fn take_while<'a>(
  cond: impl Fn(char) -> bool,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  substring(skip_while(cond))
}

/// Byte-based variant of [`take_while`]; see [`skip_while_byte`] for the
/// constraint on `cond`.
#[inline]
pub fn take_while_byte<'a>(
  cond: impl Fn(u8) -> bool,
) -> impl Fn(&'a str) -> ParseResult<'a, &'a str> {
  move |input| {
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() && cond(bytes[i]) {
      i += 1;
    }
    Ok((&input[i..], &input[..i]))
  }
}

/// Maps a success to `Some(T)` and a backtrace to `None`.
#[inline]
pub fn maybe<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Option<O>> {
  move |input| match combinator(input) {
    Ok((input, value)) => Ok((input, Some(value))),
    Err(ParseError::Backtrace) => Ok((input, None)),
    Err(err) => Err(err),
  }
}

/// Maps the success of a combinator by a function.
#[inline]
pub fn map<'a, O, R>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  func: impl Fn(O) -> R,
) -> impl Fn(&'a str) -> ParseResult<'a, R> {
  move |input| {
    let (input, result) = combinator(input)?;
    Ok((input, func(result)))
  }
}

/// Maps the result of a combinator by a function.
#[inline]
pub fn map_res<'a, O, R>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  func: impl Fn(ParseResult<'a, O>) -> R,
) -> impl Fn(&'a str) -> R {
  move |input| func(combinator(input))
}

/// Checks for either to match.
#[inline]
pub fn or<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| match a(input) {
    Ok(result) => Ok(result),
    Err(ParseError::Backtrace) => b(input),
    Err(err) => Err(err),
  }
}

/// Checks for any to match.
#[inline]
pub fn or3<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
  c: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  or(a, or(b, c))
}

/// Checks for any to match.
#[inline]
pub fn or4<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
  c: impl Fn(&'a str) -> ParseResult<'a, O>,
  d: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  or3(a, b, or(c, d))
}

/// Checks for any to match.
#[inline]
pub fn or5<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
  c: impl Fn(&'a str) -> ParseResult<'a, O>,
  d: impl Fn(&'a str) -> ParseResult<'a, O>,
  e: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  or4(a, b, c, or(d, e))
}

/// Checks for any to match.
#[inline]
pub fn or6<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
  c: impl Fn(&'a str) -> ParseResult<'a, O>,
  d: impl Fn(&'a str) -> ParseResult<'a, O>,
  e: impl Fn(&'a str) -> ParseResult<'a, O>,
  f: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  or5(a, b, c, d, or(e, f))
}

/// Checks for any to match.
#[inline]
pub fn or7<'a, O>(
  a: impl Fn(&'a str) -> ParseResult<'a, O>,
  b: impl Fn(&'a str) -> ParseResult<'a, O>,
  c: impl Fn(&'a str) -> ParseResult<'a, O>,
  d: impl Fn(&'a str) -> ParseResult<'a, O>,
  e: impl Fn(&'a str) -> ParseResult<'a, O>,
  f: impl Fn(&'a str) -> ParseResult<'a, O>,
  g: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  or6(a, b, c, d, e, or(f, g))
}

/// Returns the second value and discards the first.
#[inline]
pub fn preceded<'a, First, Second>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
) -> impl Fn(&'a str) -> ParseResult<'a, Second> {
  map(pair(first, second), |(_, second)| second)
}

/// Returns the first value and discards the second.
#[inline]
pub fn terminated<'a, First, Second>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
) -> impl Fn(&'a str) -> ParseResult<'a, First> {
  map(pair(first, second), |(first, _)| first)
}

/// Gets a second value that is delimited by a first and third.
#[inline]
pub fn delimited<'a, First, Second, Third>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
  third: impl Fn(&'a str) -> ParseResult<'a, Third>,
) -> impl Fn(&'a str) -> ParseResult<'a, Second> {
  move |input| {
    let (input, _) = first(input)?;
    let (input, return_value) = second(input)?;
    let (input, _) = third(input)?;
    Ok((input, return_value))
  }
}

/// Returns both results of the two combinators.
#[inline]
pub fn pair<'a, First, Second>(
  first: impl Fn(&'a str) -> ParseResult<'a, First>,
  second: impl Fn(&'a str) -> ParseResult<'a, Second>,
) -> impl Fn(&'a str) -> ParseResult<'a, (First, Second)> {
  move |input| {
    let (input, first_value) = first(input)?;
    let (input, second_value) = second(input)?;
    Ok((input, (first_value, second_value)))
  }
}

/// Asserts that a combinator resolves. If backtracing occurs, returns a failure.
pub fn assert_exists<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  assert(combinator, |result| result.is_ok(), message)
}

/// Asserts that a given condition is true about the combinator.
/// Otherwise returns an error with the message.
pub fn assert<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(Result<&(&'a str, O), &ParseError<'a>>) -> bool,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    let result = combinator(input);
    if condition(result.as_ref()) {
      result
    } else {
      match result {
        Err(ParseError::Failure(err)) => {
          let capacity = message.len() + 2 + err.message.len();
          let mut new_message = String::with_capacity(capacity);
          new_message.push_str(message);
          new_message.push_str("\n\n");
          new_message.push_str(&err.message);
          debug_assert_eq!(capacity, new_message.len());
          ParseError::fail(err.input, Cow::Owned(new_message))
        }
        _ => ParseError::fail(input, message),
      }
    }
  }
}

/// Changes the input on a failure in order to provide
/// a better error message.
pub fn with_failure_input<'a, O>(
  new_input: &'a str,
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    let result = combinator(input);
    match result {
      Err(ParseError::Failure(mut err)) => {
        err.input = new_input;
        Err(ParseError::Failure(err))
      }
      _ => result,
    }
  }
}

/// Provides some context to a failure.
pub fn with_error_context<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  message: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| match combinator(input) {
    Ok(result) => Ok(result),
    Err(ParseError::Backtrace) => Err(ParseError::Backtrace),
    Err(ParseError::Failure(err)) => {
      let capacity = message.len() + 2 + err.message.len();
      let mut new_message = String::with_capacity(capacity);
      new_message.push_str(message);
      new_message.push_str("\n\n");
      new_message.push_str(&err.message);
      debug_assert_eq!(new_message.len(), capacity);
      ParseError::fail(err.input, Cow::Owned(new_message))
    }
  }
}

/// Keeps consuming a combinator into an array until a condition
/// is met or backtracing occurs.
///
/// If you don't actually need the collected `Vec`, prefer
/// [`many_till_fold`] / [`many0_count`] to skip the allocation.
#[inline]
pub fn many_till<'a, O, OCondition>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(&'a str) -> ParseResult<'a, OCondition>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results: Vec<O> = Vec::new();
    while !input.is_empty() && is_backtrace(condition(input))? {
      match combinator(input) {
        Ok((result_input, value)) => {
          results.push(value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => {
          return Ok((input, results));
        }
        Err(err) => return Err(err),
      }
    }
    Ok((input, results))
  }
}

/// Keeps consuming a combinator into an array until a condition
/// is met or backtracing occurs.
///
/// If you don't need the collected `Vec`, prefer [`separated_fold`] /
/// [`separated_count`] to skip the allocation.
#[inline]
pub fn separated_list<'a, O, OSeparator>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  separator: impl Fn(&'a str) -> ParseResult<'a, OSeparator>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results: Vec<O> = Vec::new();
    while !input.is_empty() {
      match combinator(input) {
        Ok((result_input, value)) => {
          results.push(value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => {
          return Ok((input, results));
        }
        Err(err) => return Err(err),
      }
      input = match separator(input) {
        Ok((input, _)) => input,
        Err(ParseError::Backtrace) => break,
        Err(err) => return Err(err),
      };
    }
    Ok((input, results))
  }
}

/// Like [`many_till`] but folds each parsed item into an accumulator
/// instead of allocating a `Vec`. Use this when you only need a derived
/// value (e.g. a count, a sum, or a custom collection).
#[inline]
pub fn many_till_fold<'a, O, OCondition, Acc>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(&'a str) -> ParseResult<'a, OCondition>,
  init: Acc,
  fold: impl Fn(Acc, O) -> Acc,
) -> impl FnOnce(&'a str) -> ParseResult<'a, Acc> {
  move |mut input| {
    let mut acc = init;
    while !input.is_empty() && is_backtrace(condition(input))? {
      match combinator(input) {
        Ok((result_input, value)) => {
          acc = fold(acc, value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => return Ok((input, acc)),
        Err(err) => return Err(err),
      }
    }
    Ok((input, acc))
  }
}

/// Like [`separated_list`] but folds each parsed item into an accumulator
/// instead of allocating a `Vec`.
#[inline]
pub fn separated_fold<'a, O, OSeparator, Acc>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  separator: impl Fn(&'a str) -> ParseResult<'a, OSeparator>,
  init: Acc,
  fold: impl Fn(Acc, O) -> Acc,
) -> impl FnOnce(&'a str) -> ParseResult<'a, Acc> {
  move |mut input| {
    let mut acc = init;
    while !input.is_empty() {
      match combinator(input) {
        Ok((result_input, value)) => {
          acc = fold(acc, value);
          input = result_input;
        }
        Err(ParseError::Backtrace) => return Ok((input, acc)),
        Err(err) => return Err(err),
      }
      input = match separator(input) {
        Ok((input, _)) => input,
        Err(ParseError::Backtrace) => break,
        Err(err) => return Err(err),
      };
    }
    Ok((input, acc))
  }
}

/// Counts how many times the combinator matches before a backtrace.
/// Equivalent to `many0(...).len()` but does not allocate.
#[inline]
pub fn many0_count<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, usize> {
  move |input| {
    many_till_fold(
      &combinator,
      |_| ParseError::backtrace::<()>(),
      0usize,
      |acc, _| acc + 1,
    )(input)
  }
}

/// Counts how many separated items the combinator matches.
/// Equivalent to `separated_list(...).len()` but does not allocate.
#[inline]
pub fn separated_count<'a, O, OSeparator>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  separator: impl Fn(&'a str) -> ParseResult<'a, OSeparator>,
) -> impl Fn(&'a str) -> ParseResult<'a, usize> {
  move |input| {
    separated_fold(&combinator, &separator, 0usize, |acc, _| acc + 1)(input)
  }
}

/// Applies the combinator 0 or more times and returns a vector
/// of all the parsed results.
#[inline]
pub fn many0<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  many_till(combinator, |_| ParseError::backtrace::<()>())
}

/// Applies the combinator at least 1 time, but maybe more
/// and returns a vector of all the parsed results.
#[inline]
pub fn many1<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  if_not_empty(many0(combinator))
}

/// Skips the whitespace.
#[inline]
pub fn skip_whitespace(input: &str) -> ParseResult<'_, ()> {
  // ASCII fast path: whitespace in semver-like grammars is overwhelmingly
  // ASCII space or tab. Walk bytes directly until we hit non-whitespace or
  // a non-ASCII byte (rare — fall back to the char-aware loop for that).
  let bytes = input.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    let b = bytes[i];
    if b < 0x80 {
      if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
        i += 1;
      } else {
        return Ok((&input[i..], ()));
      }
    } else {
      return match whitespace(&input[i..]) {
        Ok((rest, _)) => Ok((rest, ())),
        Err(ParseError::Backtrace) => Ok((&input[i..], ())),
        Err(err) => Err(err),
      };
    }
  }
  Ok((&input[i..], ()))
}

/// Parses and expects whitespace.
#[inline]
pub fn whitespace(input: &str) -> ParseResult<'_, &str> {
  if input.is_empty() {
    return ParseError::backtrace();
  }

  // ASCII fast path mirroring `skip_whitespace`. Falls through to the
  // char-aware loop only when the first byte is non-ASCII.
  let bytes = input.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    let b = bytes[i];
    if b < 0x80 {
      if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C) {
        i += 1;
        continue;
      }
      if i == 0 {
        return ParseError::backtrace();
      }
      return Ok((&input[i..], &input[..i]));
    }
    // non-ASCII byte: hand off to char-aware scan from this position
    return whitespace_char_path(input, i);
  }
  Ok(("", input))
}

#[inline(never)]
fn whitespace_char_path(input: &str, start: usize) -> ParseResult<'_, &str> {
  for (rel_pos, c) in input[start..].char_indices() {
    if !c.is_whitespace() {
      let pos = start + rel_pos;
      if pos == 0 {
        return ParseError::backtrace();
      }
      return Ok((&input[pos..], &input[..pos]));
    }
  }
  Ok(("", input))
}

/// Checks if a condition is true for a combinator.
#[inline]
pub fn if_true<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
  condition: impl Fn(&O) -> bool,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    let (input, value) = combinator(input)?;
    if condition(&value) {
      Ok((input, value))
    } else {
      ParseError::backtrace()
    }
  }
}

pub trait IsEmptyable {
  fn is_empty(&self) -> bool;
}

impl IsEmptyable for String {
  fn is_empty(&self) -> bool {
    self.is_empty()
  }
}

impl IsEmptyable for &str {
  fn is_empty(&self) -> bool {
    (*self).is_empty()
  }
}

impl<T> IsEmptyable for Vec<T> {
  fn is_empty(&self) -> bool {
    self.is_empty()
  }
}

/// Checks if the combinator result is not empty.
#[inline]
pub fn if_not_empty<'a, R: IsEmptyable>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, R>,
) -> impl Fn(&'a str) -> ParseResult<'a, R> {
  if_true(combinator, |items| !items.is_empty())
}

/// Checks if a combinator is false without consuming the input.
#[inline]
pub fn check_not<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, ()> {
  move |input| match combinator(input) {
    Ok(_) => ParseError::backtrace(),
    Err(_) => Ok((input, ())),
  }
}

/// Logs the result for quick debugging purposes.
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub fn log_result<'a, O: std::fmt::Debug>(
  prefix: &'static str,
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, O> {
  move |input| {
    let result = combinator(input);
    println!("{} (input):  {:?}", prefix, input);
    println!("{} (result): {:#?}", prefix, result);
    result
  }
}

#[inline]
fn is_backtrace<O>(result: ParseResult<O>) -> Result<bool, ParseError> {
  match result {
    Ok(_) => Ok(false),
    Err(ParseError::Backtrace) => Ok(true),
    Err(err) => Err(err),
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn error_with_code_formatting() {
    let error = ParseErrorFailureError {
      message: "Message.".into(),
      code_snippet: Some("code".to_string()),
    };
    assert_eq!(error.to_string(), "Message.\n  code\n  ~");
  }

  #[test]
  fn error_without_code_formatting() {
    let error = ParseErrorFailureError {
      message: "Just a message.".into(),
      code_snippet: None,
    };
    assert_eq!(error.to_string(), "Just a message.");
  }

  #[test]
  fn error_new_helper_borrowed() {
    let error = ParseErrorFailureError::new("oops");
    assert_eq!(error.message, "oops");
    assert!(matches!(error.message, Cow::Borrowed(_)));
    assert!(error.code_snippet.is_none());
  }

  #[test]
  fn error_new_helper_owned() {
    let error = ParseErrorFailureError::new(String::from("runtime"));
    assert_eq!(error.message, "runtime");
    assert!(matches!(error.message, Cow::Owned(_)));
  }

  #[test]
  fn parse_error_failure_keeps_static_message_borrowed() {
    let failure = ParseErrorFailure::new("input", "static");
    assert!(matches!(failure.message, Cow::Borrowed(_)));
  }

  #[test]
  fn parse_error_failure_truncates_long_input_at_60_chars() {
    let long = "a".repeat(100);
    let failure = ParseErrorFailure::new(&long, "msg");
    let error = failure.into_error();
    assert_eq!(error.code_snippet.as_deref(), Some("a".repeat(60).as_str()));
  }

  #[test]
  fn parse_error_failure_keeps_short_input_intact() {
    let failure = ParseErrorFailure::new("short", "msg");
    let error = failure.into_error();
    assert_eq!(error.code_snippet.as_deref(), Some("short"));
    assert_eq!(error.message, "msg");
  }

  #[test]
  fn parse_error_failure_truncates_at_char_boundary() {
    // 4-byte char repeated past 60 chars should not panic and should slice
    // at a char boundary.
    let s: String = "💯".repeat(80);
    let failure = ParseErrorFailure::new(&s, "msg");
    let error = failure.into_error();
    let snippet = error.code_snippet.unwrap();
    assert_eq!(snippet.chars().count(), 60);
  }

  #[test]
  fn parse_error_failure_new_for_trailing_input() {
    let failure = ParseErrorFailure::new_for_trailing_input("rest");
    assert_eq!(failure.message, "Unexpected character.");
    assert_eq!(failure.input, "rest");
  }

  #[test]
  fn ch_matches() {
    let parser = ch('a');
    assert_eq!(parser("abc").unwrap(), ("bc", 'a'));
  }

  #[test]
  fn ch_backtraces_on_mismatch() {
    let parser = ch('a');
    assert!(matches!(parser("xyz"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn ch_backtraces_on_empty() {
    let parser = ch('a');
    assert!(matches!(parser(""), Err(ParseError::Backtrace)));
  }

  #[test]
  fn ch_handles_multibyte_char() {
    let parser = ch('💯');
    let (rest, c) = parser("💯ok").unwrap();
    assert_eq!(rest, "ok");
    assert_eq!(c, '💯');
  }

  #[test]
  fn next_char_returns_first_char() {
    assert_eq!(next_char("hello").unwrap(), ("ello", 'h'));
  }

  #[test]
  fn next_char_handles_multibyte() {
    let (rest, c) = next_char("ñoño").unwrap();
    assert_eq!(c, 'ñ');
    assert_eq!(rest, "oño");
  }

  #[test]
  fn next_char_backtraces_on_empty() {
    assert!(matches!(next_char(""), Err(ParseError::Backtrace)));
  }

  #[test]
  fn one_of_matches_ascii() {
    let parser = one_of("abc");
    assert_eq!(parser("ahi").unwrap(), ("hi", 'a'));
    assert_eq!(parser("cyz").unwrap(), ("yz", 'c'));
  }

  #[test]
  fn one_of_backtraces_when_not_in_set() {
    let parser = one_of("abc");
    assert!(matches!(parser("zoo"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn one_of_handles_non_ascii() {
    let parser = one_of("éñ");
    assert_eq!(parser("éclair").unwrap(), ("clair", 'é'));
    assert!(matches!(parser("xyz"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn tag_matches_prefix() {
    let parser = tag("hello");
    assert_eq!(parser("hello world").unwrap(), (" world", "hello"));
  }

  #[test]
  fn tag_backtraces_on_mismatch() {
    let parser = tag("hello");
    assert!(matches!(parser("world"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn tag_returns_slice_from_input() {
    // The returned &str should be a slice of the input, not the static tag.
    let parser = tag("ab");
    let input = "abcd";
    let (_, matched) = parser(input).unwrap();
    assert!(std::ptr::eq(matched.as_ptr(), input.as_ptr()));
  }

  #[test]
  fn tag_owned_matches_prefix() {
    let parser = tag_owned("foo".to_string());
    assert_eq!(parser("foobar").unwrap(), ("bar", "foo"));
  }

  #[test]
  fn tag_owned_backtraces_on_mismatch() {
    let parser = tag_owned("foo".to_string());
    assert!(matches!(parser("bar"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn substring_returns_consumed_text() {
    let parser = substring(tag("ab"));
    assert_eq!(parser("abcd").unwrap(), ("cd", "ab"));
  }

  #[test]
  fn substring_propagates_backtrace() {
    let parser = substring(tag("ab"));
    assert!(matches!(parser("xy"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn skip_while_skips_matching_chars() {
    let parser = skip_while(|c| c.is_ascii_digit());
    assert_eq!(parser("123abc").unwrap(), ("abc", ()));
  }

  #[test]
  fn skip_while_skips_nothing_when_none_match() {
    let parser = skip_while(|c| c.is_ascii_digit());
    assert_eq!(parser("abc").unwrap(), ("abc", ()));
  }

  #[test]
  fn skip_while_consumes_entire_input() {
    let parser = skip_while(|c| c.is_ascii_digit());
    assert_eq!(parser("12345").unwrap(), ("", ()));
  }

  #[test]
  fn take_while_returns_matched_prefix() {
    let parser = take_while(|c| c.is_ascii_alphabetic());
    assert_eq!(parser("abc123").unwrap(), ("123", "abc"));
  }

  #[test]
  fn take_while_returns_empty_when_none_match() {
    let parser = take_while(|c| c.is_ascii_alphabetic());
    assert_eq!(parser("123").unwrap(), ("123", ""));
  }

  #[test]
  fn maybe_wraps_success_in_some() {
    let parser = maybe(tag("ab"));
    assert_eq!(parser("abcd").unwrap(), ("cd", Some("ab")));
  }

  #[test]
  fn maybe_returns_none_on_backtrace() {
    let parser = maybe(tag("ab"));
    assert_eq!(parser("xy").unwrap(), ("xy", None));
  }

  #[test]
  fn maybe_propagates_failure() {
    let parser = maybe(|input: &str| ParseError::fail::<()>(input, "boom"));
    assert!(matches!(parser("xy"), Err(ParseError::Failure(_))));
  }

  #[test]
  fn map_transforms_result() {
    let parser = map(tag("42"), |s: &str| s.parse::<u32>().unwrap());
    assert_eq!(parser("42rest").unwrap(), ("rest", 42));
  }

  #[test]
  fn map_propagates_backtrace() {
    let parser = map(tag("42"), |s: &str| s.to_string());
    assert!(matches!(parser("xyz"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn map_res_can_recover_from_backtrace() {
    let parser = map_res(tag("ab"), |r| match r {
      Ok((rest, _)) => (rest, "matched"),
      Err(_) => ("fallback", "missed"),
    });
    assert_eq!(parser("abcd"), ("cd", "matched"));
    assert_eq!(parser("xyzz"), ("fallback", "missed"));
  }

  #[test]
  fn or_picks_first_when_it_matches() {
    let parser = or(tag("ab"), tag("cd"));
    assert_eq!(parser("abXX").unwrap(), ("XX", "ab"));
  }

  #[test]
  fn or_falls_through_on_backtrace() {
    let parser = or(tag("ab"), tag("cd"));
    assert_eq!(parser("cdXX").unwrap(), ("XX", "cd"));
  }

  #[test]
  fn or_backtraces_when_neither_matches() {
    let parser = or(tag("ab"), tag("cd"));
    assert!(matches!(parser("zz"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn or_propagates_failure_from_first() {
    let parser = or(
      |input: &str| ParseError::fail::<&str>(input, "boom"),
      tag("cd"),
    );
    assert!(matches!(parser("cd"), Err(ParseError::Failure(_))));
  }

  #[test]
  fn or3_through_or7_match_each_branch() {
    let p3 = or3(tag("a"), tag("b"), tag("c"));
    assert_eq!(p3("c!").unwrap().1, "c");

    let p4 = or4(tag("a"), tag("b"), tag("c"), tag("d"));
    assert_eq!(p4("d!").unwrap().1, "d");

    let p5 = or5(tag("a"), tag("b"), tag("c"), tag("d"), tag("e"));
    assert_eq!(p5("e!").unwrap().1, "e");

    let p6 = or6(tag("a"), tag("b"), tag("c"), tag("d"), tag("e"), tag("f"));
    assert_eq!(p6("f!").unwrap().1, "f");

    let p7 = or7(
      tag("a"),
      tag("b"),
      tag("c"),
      tag("d"),
      tag("e"),
      tag("f"),
      tag("g"),
    );
    assert_eq!(p7("g!").unwrap().1, "g");
    assert!(matches!(p7("zz"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn preceded_drops_first_keeps_second() {
    let parser = preceded(tag("("), tag("x"));
    assert_eq!(parser("(x)").unwrap(), (")", "x"));
  }

  #[test]
  fn terminated_keeps_first_drops_second() {
    let parser = terminated(tag("x"), tag(")"));
    assert_eq!(parser("x)rest").unwrap(), ("rest", "x"));
  }

  #[test]
  fn delimited_returns_inner() {
    let parser = delimited(tag("("), tag("x"), tag(")"));
    assert_eq!(parser("(x)rest").unwrap(), ("rest", "x"));
  }

  #[test]
  fn delimited_backtraces_when_third_missing() {
    let parser = delimited(tag("("), tag("x"), tag(")"));
    assert!(matches!(parser("(x!"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn pair_returns_both_values() {
    let parser = pair(tag("ab"), tag("cd"));
    assert_eq!(parser("abcd!").unwrap(), ("!", ("ab", "cd")));
  }

  #[test]
  fn pair_backtraces_when_second_fails() {
    let parser = pair(tag("ab"), tag("cd"));
    assert!(matches!(parser("abxx"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn assert_exists_promotes_backtrace_to_failure() {
    let parser = assert_exists(tag("ab"), "expected ab");
    match parser("xx") {
      Err(ParseError::Failure(err)) => assert_eq!(err.message, "expected ab"),
      other => panic!("expected failure, got {:?}", other),
    }
  }

  #[test]
  fn assert_exists_passes_through_success() {
    let parser = assert_exists(tag("ab"), "expected ab");
    assert_eq!(parser("abcd").unwrap(), ("cd", "ab"));
  }

  #[test]
  fn assert_appends_existing_failure_message() {
    fn inner(input: &str) -> ParseResult<'_, ()> {
      ParseError::fail(input, "inner")
    }
    let parser = assert(inner, |r| r.is_ok(), "outer");
    match parser("input") {
      Err(ParseError::Failure(err)) => {
        assert_eq!(err.message, "outer\n\ninner");
      }
      other => panic!("expected failure, got {:?}", other),
    }
  }

  #[test]
  fn with_failure_input_replaces_input_on_failure() {
    fn inner(input: &str) -> ParseResult<'_, ()> {
      ParseError::fail(input, "msg")
    }
    let parser = with_failure_input("override", inner);
    match parser("original") {
      Err(ParseError::Failure(err)) => assert_eq!(err.input, "override"),
      other => panic!("expected failure, got {:?}", other),
    }
  }

  #[test]
  fn with_failure_input_does_not_touch_backtrace() {
    let parser = with_failure_input("override", tag("ab"));
    assert!(matches!(parser("xx"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn with_error_context_prepends_message() {
    fn inner(input: &str) -> ParseResult<'_, ()> {
      ParseError::fail(input, "inner")
    }
    let parser = with_error_context(inner, "context");
    match parser("input") {
      Err(ParseError::Failure(err)) => {
        assert_eq!(err.message, "context\n\ninner");
      }
      other => panic!("expected failure, got {:?}", other),
    }
  }

  #[test]
  fn with_error_context_passes_through_backtrace() {
    let parser = with_error_context(tag("ab"), "context");
    assert!(matches!(parser("xx"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn many_till_collects_until_condition() {
    let parser = many_till(ch('a'), ch('!'));
    let (rest, results) = parser("aaa!rest").unwrap();
    assert_eq!(rest, "!rest");
    assert_eq!(results, vec!['a', 'a', 'a']);
  }

  #[test]
  fn many_till_stops_on_combinator_backtrace() {
    let parser = many_till(ch('a'), ch('!'));
    let (rest, results) = parser("aabb").unwrap();
    assert_eq!(rest, "bb");
    assert_eq!(results, vec!['a', 'a']);
  }

  #[test]
  fn many_till_handles_empty_input() {
    let parser = many_till(ch('a'), ch('!'));
    let (rest, results) = parser("").unwrap();
    assert_eq!(rest, "");
    assert!(results.is_empty());
  }

  #[test]
  fn separated_list_collects_with_separator() {
    let parser =
      separated_list(take_while(|c| c.is_ascii_alphabetic()), ch(','));
    let (rest, results) = parser("a,bb,ccc!").unwrap();
    assert_eq!(rest, "!");
    assert_eq!(results, vec!["a", "bb", "ccc"]);
  }

  #[test]
  fn separated_list_handles_no_separator() {
    let parser = separated_list(tag("ab"), ch(','));
    let (rest, results) = parser("ab!").unwrap();
    assert_eq!(rest, "!");
    assert_eq!(results, vec!["ab"]);
  }

  #[test]
  fn many0_returns_empty_when_none_match() {
    let parser = many0(ch('a'));
    let (rest, results) = parser("xxx").unwrap();
    assert_eq!(rest, "xxx");
    assert!(results.is_empty());
  }

  #[test]
  fn many0_collects_all_matches() {
    let parser = many0(ch('a'));
    let (rest, results) = parser("aaab").unwrap();
    assert_eq!(rest, "b");
    assert_eq!(results, vec!['a', 'a', 'a']);
  }

  #[test]
  fn many1_requires_at_least_one() {
    let parser = many1(ch('a'));
    assert!(matches!(parser("xxx"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn many1_collects_when_at_least_one() {
    let parser = many1(ch('a'));
    let (rest, results) = parser("aab").unwrap();
    assert_eq!(rest, "b");
    assert_eq!(results, vec!['a', 'a']);
  }

  #[test]
  fn skip_whitespace_skips_leading_whitespace() {
    assert_eq!(skip_whitespace("   hi").unwrap(), ("hi", ()));
  }

  #[test]
  fn skip_whitespace_succeeds_with_no_whitespace() {
    assert_eq!(skip_whitespace("hi").unwrap(), ("hi", ()));
  }

  #[test]
  fn skip_whitespace_succeeds_on_empty() {
    assert_eq!(skip_whitespace("").unwrap(), ("", ()));
  }

  #[test]
  fn whitespace_returns_consumed_whitespace() {
    assert_eq!(whitespace("   hi").unwrap(), ("hi", "   "));
  }

  #[test]
  fn whitespace_consumes_all_when_only_whitespace() {
    assert_eq!(whitespace("   ").unwrap(), ("", "   "));
  }

  #[test]
  fn whitespace_backtraces_when_first_char_is_not_whitespace() {
    assert!(matches!(whitespace("hi"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn whitespace_backtraces_on_empty() {
    assert!(matches!(whitespace(""), Err(ParseError::Backtrace)));
  }

  #[test]
  fn if_true_passes_when_condition_holds() {
    let parser = if_true(tag("ab"), |s: &&str| s.len() == 2);
    assert_eq!(parser("abcd").unwrap(), ("cd", "ab"));
  }

  #[test]
  fn if_true_backtraces_when_condition_fails() {
    let parser = if_true(tag("ab"), |s: &&str| s.len() == 99);
    assert!(matches!(parser("abcd"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn if_not_empty_rejects_empty_results() {
    let parser = if_not_empty(take_while(|c| c.is_ascii_digit()));
    assert!(matches!(parser("abc"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn if_not_empty_accepts_non_empty_results() {
    let parser = if_not_empty(take_while(|c| c.is_ascii_digit()));
    assert_eq!(parser("12abc").unwrap(), ("abc", "12"));
  }

  #[test]
  fn check_not_succeeds_without_consuming_when_combinator_fails() {
    let parser = check_not(tag("ab"));
    assert_eq!(parser("xy").unwrap(), ("xy", ()));
  }

  #[test]
  fn check_not_backtraces_when_combinator_succeeds() {
    let parser = check_not(tag("ab"));
    assert!(matches!(parser("ab!"), Err(ParseError::Backtrace)));
  }

  #[test]
  fn is_emptyable_string() {
    let s = String::from("");
    assert!(IsEmptyable::is_empty(&s));
    let s = String::from("x");
    assert!(!IsEmptyable::is_empty(&s));
  }

  #[test]
  fn is_emptyable_str() {
    let s: &str = "";
    assert!(IsEmptyable::is_empty(&s));
    let s: &str = "x";
    assert!(!IsEmptyable::is_empty(&s));
  }

  #[test]
  fn is_emptyable_vec() {
    let v: Vec<i32> = vec![];
    assert!(IsEmptyable::is_empty(&v));
    let v = vec![1];
    assert!(!IsEmptyable::is_empty(&v));
  }

  #[test]
  fn with_failure_handling_returns_value_when_fully_consumed() {
    let parser = with_failure_handling(tag("hello"));
    assert_eq!(parser("hello").unwrap(), "hello");
  }

  #[test]
  fn with_failure_handling_errors_on_trailing_input() {
    let parser = with_failure_handling(tag("hello"));
    let err = parser("hello!").unwrap_err();
    assert_eq!(err.message, "Unexpected character.");
  }

  #[test]
  fn with_failure_handling_errors_on_backtrace() {
    let parser = with_failure_handling(tag("hello"));
    let err = parser("nope").unwrap_err();
    assert_eq!(err.message, "Unexpected character.");
  }

  #[test]
  fn skip_while_byte_skips_matching_bytes() {
    let parser = skip_while_byte(|b| b.is_ascii_digit());
    assert_eq!(parser("123abc").unwrap(), ("abc", ()));
  }

  #[test]
  fn skip_while_byte_consumes_entire_input() {
    let parser = skip_while_byte(|b| b.is_ascii_digit());
    assert_eq!(parser("12345").unwrap(), ("", ()));
  }

  #[test]
  fn skip_while_byte_skips_nothing_when_none_match() {
    let parser = skip_while_byte(|b| b.is_ascii_digit());
    assert_eq!(parser("abc").unwrap(), ("abc", ()));
  }

  #[test]
  fn take_while_byte_returns_matched_prefix() {
    let parser = take_while_byte(|b| b.is_ascii_alphabetic());
    assert_eq!(parser("abc123").unwrap(), ("123", "abc"));
  }

  #[test]
  fn take_while_byte_returns_empty_when_none_match() {
    let parser = take_while_byte(|b| b.is_ascii_alphabetic());
    assert_eq!(parser("123").unwrap(), ("123", ""));
  }

  #[test]
  fn many_till_fold_sums_without_allocating() {
    let parser = many_till_fold(
      take_while(|c| c.is_ascii_digit()),
      ch('!'),
      0u32,
      |acc, s: &str| acc + s.parse::<u32>().unwrap_or(0),
    );
    let (rest, sum) = parser("1!").unwrap();
    assert_eq!(rest, "!");
    assert_eq!(sum, 1);
  }

  #[test]
  fn many0_count_counts_matches() {
    let parser = many0_count(ch('a'));
    let (rest, n) = parser("aaab").unwrap();
    assert_eq!(rest, "b");
    assert_eq!(n, 3);
  }

  #[test]
  fn many0_count_returns_zero_when_no_match() {
    let parser = many0_count(ch('a'));
    let (rest, n) = parser("xxx").unwrap();
    assert_eq!(rest, "xxx");
    assert_eq!(n, 0);
  }

  #[test]
  fn separated_fold_collects_into_string() {
    let parser = separated_fold(
      take_while(|c| c.is_ascii_alphabetic()),
      ch(','),
      String::new(),
      |mut acc, s: &str| {
        if !acc.is_empty() {
          acc.push('|');
        }
        acc.push_str(s);
        acc
      },
    );
    let (rest, joined) = parser("a,bb,ccc!").unwrap();
    assert_eq!(rest, "!");
    assert_eq!(joined, "a|bb|ccc");
  }

  #[test]
  fn separated_count_counts_items() {
    let parser =
      separated_count(take_while(|c| c.is_ascii_alphabetic()), ch(','));
    let (rest, n) = parser("a,bb,ccc!").unwrap();
    assert_eq!(rest, "!");
    assert_eq!(n, 3);
  }

  #[test]
  fn skip_whitespace_handles_non_ascii_whitespace() {
    // U+00A0 NO-BREAK SPACE is whitespace but non-ASCII; ensure the
    // ASCII fast path correctly hands off to the char-aware fallback.
    let input = "\u{00A0}hi";
    assert_eq!(skip_whitespace(input).unwrap(), ("hi", ()));
  }

  #[test]
  fn whitespace_handles_non_ascii_whitespace() {
    // leading non-ASCII whitespace must still be consumed
    let input = "\u{00A0}hi";
    let (rest, ws) = whitespace(input).unwrap();
    assert_eq!(rest, "hi");
    assert_eq!(ws, "\u{00A0}");
  }

  #[test]
  fn with_failure_handling_propagates_failure_message() {
    let parser = with_failure_handling(|input: &str| {
      ParseError::fail::<()>(input, "custom failure")
    });
    let err = parser("anything").unwrap_err();
    assert_eq!(err.message, "custom failure");
  }
}
