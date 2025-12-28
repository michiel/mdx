# mdx

## Installation

This repository is a Cargo workspace, so `cargo install --path .` fails with a
virtual manifest error. Install the `mdx` package from its crate directory:

```bash
cargo install --path mdx
```

If you want a debug build without installing, run:

```bash
cargo run -p mdx -- --help
```
