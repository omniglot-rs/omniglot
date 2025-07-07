# Omniglot

Omniglot is framework that allows Rust programs to safely and
efficiently interact with foreign libraries in any language.

## Runtime Crates

This repository contains the core Omniglot crate. But to actually use
Omniglot, you use on a library specialized for a particular
runtime. Currently supported runtimes include:

- [Tock kernel (using RISC-V PMP)](https://github.com/omniglot-rs/omniglot-tock)

- [Linux userspace (using x86 MPK)](https://github.com/omniglot-rs/omniglot-mpk)

## TODO

- [ ] Documentation
- [ ] Quick start guide
- [ ] Tock Cortex-M support
- [ ] Improved `bindgen` integration and calling convention analysis.

## Publications

Omniglot was published at [USENIX OSDI
2025](https://www.usenix.org/conference/osdi25/presentation/schuermann),
where it won a best paper award.

USENIX ;login; also
[published](https://www.usenix.org/publications/loginonline/memory-safety-merely-table-stakes)
an article motivating and describing the work.

## Reproducibility

To reproduce the experiments and results from our OSDI'25 paper, we
publish our reproduction instructions and code artifacts on Zenodo:
<https://doi.org/10.5281/zenodo.15602886>


## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
