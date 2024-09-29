# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# 0.2.0 (29. September, 2024)

This is usability wise not a breaking release,
however it does make changes to the API which might break subtle edge cases
and it also increases the MSRV to 1.75.

New Features:

- add a delay (Duration) that can be used
  to trigger the cancel notification to ongoing jobs once the shutdown trigger (signal)
  has been received;
- add a second signal factory that can be used to create an overwrite
  signal to be created and triggered once the main signal has been triggered,
  as an alternative to the jobs being complete or max delay has been reached.

Both features can be configured using the newly introduced `ShutdownBuilder`,
which can be made directly or via `Shutdown::builder`.

# 0.1.6 (01. December, 2023)

- Upgrade hyper examples to adapt to dev dependency hyper v1.0 (was hyper v0.14);

# 0.1.5 (20. September, 2023)

- Support and use Loom for testing;
 - Fixes a bug in the private trigger code where a race condition could cause a deadlock (found using loom);
- Signal / Project support for the Windows platform;
  - affected code: `crate::default_signal` and `crate::Shutdown::default`;
    - Unix and Windows are supported and have this code enabled;
    - Other platforms won't have this code;
    - When using Loom this code is also not there;
  - This fixes build errors for platforms that we do not support for the default signal;

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
