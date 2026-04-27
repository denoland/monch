use divan::Bencher;
use monch::ParseError;
use monch::ParseResult;
use monch::ch;

fn main() {
  divan::main();
}

// version with `Vec::new()` — no upfront alloc, grows on push.
fn many0_no_cap<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results: Vec<O> = Vec::new();
    while !input.is_empty() {
      match combinator(input) {
        Ok((rest, value)) => {
          results.push(value);
          input = rest;
        }
        Err(ParseError::Backtrace) => return Ok((input, results)),
        Err(e) => return Err(e),
      }
    }
    Ok((input, results))
  }
}

// version with `Vec::with_capacity(4)` — allocates 4 slots upfront.
fn many0_cap4<'a, O>(
  combinator: impl Fn(&'a str) -> ParseResult<'a, O>,
) -> impl Fn(&'a str) -> ParseResult<'a, Vec<O>> {
  move |mut input| {
    let mut results: Vec<O> = Vec::with_capacity(4);
    while !input.is_empty() {
      match combinator(input) {
        Ok((rest, value)) => {
          results.push(value);
          input = rest;
        }
        Err(ParseError::Backtrace) => return Ok((input, results)),
        Err(e) => return Err(e),
      }
    }
    Ok((input, results))
  }
}

const ZERO: &str = "x";
const ONE: &str = "ax";
const FOUR: &str = "aaaax";
const SIXTEEN: &str = "aaaaaaaaaaaaaaaax";

mod len_0 {
  use super::*;
  #[divan::bench]
  fn no_cap(bencher: Bencher) {
    let p = many0_no_cap(ch('a'));
    bencher.bench_local(|| p(divan::black_box(ZERO)));
  }
  #[divan::bench]
  fn cap4(bencher: Bencher) {
    let p = many0_cap4(ch('a'));
    bencher.bench_local(|| p(divan::black_box(ZERO)));
  }
}

mod len_1 {
  use super::*;
  #[divan::bench]
  fn no_cap(bencher: Bencher) {
    let p = many0_no_cap(ch('a'));
    bencher.bench_local(|| p(divan::black_box(ONE)));
  }
  #[divan::bench]
  fn cap4(bencher: Bencher) {
    let p = many0_cap4(ch('a'));
    bencher.bench_local(|| p(divan::black_box(ONE)));
  }
}

mod len_4 {
  use super::*;
  #[divan::bench]
  fn no_cap(bencher: Bencher) {
    let p = many0_no_cap(ch('a'));
    bencher.bench_local(|| p(divan::black_box(FOUR)));
  }
  #[divan::bench]
  fn cap4(bencher: Bencher) {
    let p = many0_cap4(ch('a'));
    bencher.bench_local(|| p(divan::black_box(FOUR)));
  }
}

mod len_16 {
  use super::*;
  #[divan::bench]
  fn no_cap(bencher: Bencher) {
    let p = many0_no_cap(ch('a'));
    bencher.bench_local(|| p(divan::black_box(SIXTEEN)));
  }
  #[divan::bench]
  fn cap4(bencher: Bencher) {
    let p = many0_cap4(ch('a'));
    bencher.bench_local(|| p(divan::black_box(SIXTEEN)));
  }
}
