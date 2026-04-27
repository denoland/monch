use divan::Bencher;
use monch::ParseError;
use monch::ParseResult;
use monch::next_char;
use monch::one_of;

fn main() {
  divan::main();
}

// the original `String::contains`-based implementation, kept here so divan
// can compare it head-to-head against the current bitmap-based `one_of`.
fn one_of_old<'a>(
  value: &'static str,
) -> impl Fn(&'a str) -> ParseResult<'a, char> {
  move |input| {
    let (input, c) = next_char(input)?;
    if value.contains(c) {
      Ok((input, c))
    } else {
      ParseError::backtrace()
    }
  }
}

const DIGITS: &str = "0123456789";
const IDENT: &str =
  "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
const OPS: &str = "+-*/%<>=!&|^~";

// each input is one char to parse; the rest is unused tail.
const HIT_DIGIT: &str = "5_____";
const MISS_DIGIT: &str = "x_____";
const HIT_IDENT: &str = "k_____";
const MISS_IDENT: &str = "5_____";
const HIT_OP: &str = "+_____";
const MISS_OP: &str = "z_____";

mod digits_hit {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(DIGITS);
    bencher.bench_local(|| p(divan::black_box(HIT_DIGIT)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(DIGITS);
    bencher.bench_local(|| p(divan::black_box(HIT_DIGIT)));
  }
}

mod digits_miss {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(DIGITS);
    bencher.bench_local(|| p(divan::black_box(MISS_DIGIT)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(DIGITS);
    bencher.bench_local(|| p(divan::black_box(MISS_DIGIT)));
  }
}

mod ident_hit {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(IDENT);
    bencher.bench_local(|| p(divan::black_box(HIT_IDENT)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(IDENT);
    bencher.bench_local(|| p(divan::black_box(HIT_IDENT)));
  }
}

mod ident_miss {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(IDENT);
    bencher.bench_local(|| p(divan::black_box(MISS_IDENT)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(IDENT);
    bencher.bench_local(|| p(divan::black_box(MISS_IDENT)));
  }
}

mod ops_hit {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(OPS);
    bencher.bench_local(|| p(divan::black_box(HIT_OP)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(OPS);
    bencher.bench_local(|| p(divan::black_box(HIT_OP)));
  }
}

mod ops_miss {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = one_of_old(OPS);
    bencher.bench_local(|| p(divan::black_box(MISS_OP)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = one_of(OPS);
    bencher.bench_local(|| p(divan::black_box(MISS_OP)));
  }
}
