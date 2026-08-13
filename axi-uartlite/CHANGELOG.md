Change Log
=======

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

# [unreleased]

# [v0.3.0] 2026-08-12

- Removed `unsafe` for Async TX constructor again, the special case does not warrant an `unsafe`
  attribute.
- Adds optional `portable-atomic` feature for portable atomic operations
- Renamed the `registers` module to `regs`; register field types now live in `regs::fields`.
- Re-export `Status` at the crate root, since it appears in the public API.

# [v0.2.1] 2026-06-08

- Fix for MSRV: v1.87.

# [v0.2.0] 2026-06-08

- TX futures borrow buffer for their lifetime now.
- Constructor is now `unsafe`.
- Async TX write method now returns a future.

# [v0.1.1] 2025-11-28

Minor `Cargo.toml` tweaks

# [v0.1.0] 2025-11-28

Initial release.

[unreleased]: https://github.com/us-irs/axi-hdl-rs/compare/axi-uartlite-v0.3.0...HEAD
[v0.3.0]: https://github.com/us-irs/axi-hdl-rs/releases/tag/axi-uartlite-v0.3.0
[v0.2.1]: https://egit.irs.uni-stuttgart.de/rust/axi-uartlite/compare/v0.2.0...v0.2.1
[v0.2.0]: https://egit.irs.uni-stuttgart.de/rust/axi-uarglite/compare/v0.1.1...v0.2.0
[v0.1.1]: https://egit.irs.uni-stuttgart.de/rust/axi-uartlite/compare/v0.1.0...v0.1.1
[v0.1.0]: https://egit.irs.uni-stuttgart.de/rust/axi-uartlite/tag/v0.1.0
