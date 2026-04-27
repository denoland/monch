use divan::Bencher;

fn main() {
  divan::main();
}

mod ch {
  use super::*;
  use monch::ch;

  #[divan::bench]
  fn hit(bencher: Bencher) {
    let parser = ch('a');
    bencher.bench_local(|| parser(divan::black_box("axxxxx")));
  }

  #[divan::bench]
  fn miss(bencher: Bencher) {
    let parser = ch('a');
    bencher.bench_local(|| parser(divan::black_box("bxxxxx")));
  }
}

mod tag {
  use super::*;
  use monch::tag;
  use monch::tag_owned;

  #[divan::bench]
  fn hit(bencher: Bencher) {
    let parser = tag("hello");
    bencher.bench_local(|| parser(divan::black_box("hello world")));
  }

  #[divan::bench]
  fn miss(bencher: Bencher) {
    let parser = tag("hello");
    bencher.bench_local(|| parser(divan::black_box("howdy world")));
  }

  #[divan::bench]
  fn owned_hit(bencher: Bencher) {
    let parser = tag_owned("hello".to_string());
    bencher.bench_local(|| parser(divan::black_box("hello world")));
  }
}

mod one_of {
  use super::*;
  use monch::one_of;

  const DIGITS: &str = "0123456789";
  const IDENT: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
  const OPS: &str = "+-*/%<>=!&|^~";

  #[divan::bench]
  fn digits_hit(bencher: Bencher) {
    let parser = one_of(DIGITS);
    bencher.bench_local(|| parser(divan::black_box("5xxxxx")));
  }

  #[divan::bench]
  fn digits_miss(bencher: Bencher) {
    let parser = one_of(DIGITS);
    bencher.bench_local(|| parser(divan::black_box("xxxxxx")));
  }

  #[divan::bench]
  fn ident_hit(bencher: Bencher) {
    let parser = one_of(IDENT);
    bencher.bench_local(|| parser(divan::black_box("kxxxxx")));
  }

  #[divan::bench]
  fn ident_miss(bencher: Bencher) {
    let parser = one_of(IDENT);
    bencher.bench_local(|| parser(divan::black_box("5xxxxx")));
  }

  #[divan::bench]
  fn ops_hit(bencher: Bencher) {
    let parser = one_of(OPS);
    bencher.bench_local(|| parser(divan::black_box("+xxxxx")));
  }

  #[divan::bench]
  fn ops_miss(bencher: Bencher) {
    let parser = one_of(OPS);
    bencher.bench_local(|| parser(divan::black_box("zxxxxx")));
  }
}

mod many0 {
  use super::*;
  use monch::ch;
  use monch::many0;

  #[divan::bench]
  fn empty(bencher: Bencher) {
    let parser = many0(ch('a'));
    bencher.bench_local(|| parser(divan::black_box("xxxxx")));
  }

  #[divan::bench]
  fn short(bencher: Bencher) {
    let parser = many0(ch('a'));
    bencher.bench_local(|| parser(divan::black_box("aaax")));
  }

  #[divan::bench]
  fn long(bencher: Bencher) {
    let parser = many0(ch('a'));
    bencher.bench_local(|| parser(divan::black_box("aaaaaaaaaaaaaaaax")));
  }
}
