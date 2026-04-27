use divan::Bencher;
use monch::ParseResult;
use monch::ch;
use monch::if_true;
use monch::next_char;

fn main() {
  divan::main();
}

// the previous implementation: `if_true(next_char, ...)` — extra closure layer.
fn ch_old<'a>(c: char) -> impl Fn(&'a str) -> ParseResult<'a, char> {
  if_true(next_char, move |found_char| *found_char == c)
}

const HIT: &str = "ax";
const MISS: &str = "bx";

mod hit {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = ch_old('a');
    bencher.bench_local(|| p(divan::black_box(HIT)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = ch('a');
    bencher.bench_local(|| p(divan::black_box(HIT)));
  }
}

mod miss {
  use super::*;
  #[divan::bench]
  fn old(bencher: Bencher) {
    let p = ch_old('a');
    bencher.bench_local(|| p(divan::black_box(MISS)));
  }
  #[divan::bench]
  fn new(bencher: Bencher) {
    let p = ch('a');
    bencher.bench_local(|| p(divan::black_box(MISS)));
  }
}
