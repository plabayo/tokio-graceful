# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# 0.1.4 (08. September, 2023)

- Add example regarding ensuring you do catch exits and document it;

# 0.1.3 (07. September, 2023)

- Support and add Waitgroup example;
- Fix mistake in docs (thank you [Mike Cronce](https://github.com/mcronce));
- Update 0.1.2 changelog to highlight the library is no longer 100% Rust Safe Code;

# 0.1.2 (05. September, 2023)

- Fix typos in README (thank you [@hds](https://github.com/hds));
- Performance improvements (thank you awake readers on Reddit);
- add more docs to README and internal code;
- library is no longer 100% safe Rust code, due to usage of
  <https://doc.rust-lang.org/stable/std/mem/struct.ManuallyDrop.html> in an internal struct;

# 0.1.1 (05. September, 2023)

- Improved documentation and add FAQ to readme;
- Optimization to the `into_spawn*` methods (don't clone first);
- add CI semver check;

# 0.1.0 (04. September, 2023)

- Initial release.
