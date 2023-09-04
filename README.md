[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![MIT License][license-mit-badge]][license-mit-url]
[![Apache 2.0 License][license-apache-badge]][license-apache-url]
[![Build Status][actions-badge]][actions-url]

[![Buy Me A Coffee][bmac-badge]][bmac-url]
[![GitHub Sponsors][ghs-badge]][ghs-url]

[crates-badge]: https://img.shields.io/crates/v/tokio-graceful.svg
[crates-url]: https://crates.io/crates/tokio-graceful
[docs-badge]: https://img.shields.io/docsrs/tokio-graceful/latest
[docs-url]: https://docs.rs/tokio-graceful/latest/tokio_graceful/index.html
[license-mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[license-mit-url]: https://github.com/plabayo/tokio-graceful/blob/main/LICENSE-MIT
[license-apache-badge]: https://img.shields.io/badge/license-APACHE-blue.svg
[license-apache-url]: https://github.com/plabayo/tokio-graceful/blob/main/LICENSE-APACHE
[actions-badge]: https://github.com/plabayo/tokio-graceful/workflows/CI/badge.svg
[actions-url]: https://github.com/plabayo/tokio-graceful/actions/workflows/CI.yml?query=branch%3Amain

[bmac-badge]: https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black
[bmac-url]: https://www.buymeacoffee.com/plabayo
[ghs-badge]: https://img.shields.io/badge/sponsor-30363D?style=for-the-badge&logo=GitHub-Sponsors&logoColor=#EA4AAA
[ghs-url]: https://github.com/sponsors/plabayo

Utilities for graceful shutdown of [tokio](https://tokio.rs/) applications.

## Examples

### Runnable Examples

The runnable examples are best run with `RUST_LOG=trace` environment variable set,
such that you see the verbose logs of `tokio-graceful` and really see it in action
and get a sense on how it works, or at least its flow is.

> [examples/tokio_tcp.rs](https://github.com/plabayo/tokio-graceful/tree/main/examples/tokio_tcp.rs)
>
> ```bash
> RUST_LOG=trace cargo run --example tokio_tcp
> ```

The `tokio_tcp` example showcases the original use case of why `tokio-graceful` shutdown was developed,
as it makes managing graceful shutdown from start to finish a lot easier, without immediately grabbing
to big power tools or providing more then is needed.

The example runs a tcp 'echo' server which you can best play with using
telnet: `telnet 127.0.0.1 8080`. As you are in control of when to exit you can easily let it timeout if you wish.

> [examples/hyper.rs](https://github.com/plabayo/tokio-graceful/tree/main/examples/hyper.rs)
>
> ```bash
> RUST_LOG=trace cargo run --example hyper
> ```

In case you wish to use this library as a [Hyper](https://hyper.rs/) user
you can do so using pretty much the same approach as
the Tokio tcp example.

This example only has one router server function which returns 'hello' (200 OK) after 5s.
The delay is there to allow you to see the graceful shutdown in action.

## Contributing

:balloon: Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][contributing] to help you get involved in the
`tokio-graceful` project.

### Shoutouts

Special shoutout for this library goes to [the Tokio ecosystem](https://tokio.rs/).
Those who developed it as well as the folks hanging on the [Tokio discord server](https://discord.gg/tokio).
The discussions and Q&A sessions with them were very crucial to the development of this project.
Tokio's codebase is also a gem of examples on what is possible and what are good practices.

In this context also an extra shoutout to [@tobz](https://github.com/tobz) who
gave me the idea of approaching it from an Atomic perspective instead
of immediately going for channel solutions.

## License

This project is dual-licensed under both the [MIT license][mit-license] and [Apache 2.0 License][apache-license].

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `tokio-graceful` by you, shall be licensed as both [MIT][mit-license] and [Apache 2.0][apache-license],
without any additional terms or conditions.

[contributing]: https://github.com/plabayo/tokio-graceful/blob/main/CONTRIBUTING.md
[mit-license]: https://github.com/plabayo/tokio-graceful/blob/main/LICENSE-MIT
[apache-license]: https://github.com/plabayo/tokio-graceful/blob/main/LICENSE-APACHE

## Sponsors

tokio-graceful is **completely free, open-source software** which needs time to develop and maintain.

Support this project by becoming a [sponsor][ghs-url]. One time payments are accepted [at GitHub][ghs-url] as well as at ["Buy me a Coffee"][bmac-url]

Sponsors help us continue to maintain and improve `tokio-graceful`, as well as other
Free and Open Source (FOSS) technology. It also helps us to create
educational content such as <https://github.com/plabayo/learn-rust-101>,
and other open source libraries such as <https://github.com/plabayo/tower-async>.

Sponsors receive perks and depending on your regular contribution it also
allows you to rely on us for support and consulting.

### Contribute to Open Source

Part of the money we receive from sponsors is used to contribute to other projects
that we depend upon. Plabayo sponsors the following organisations and individuals
building and maintaining open source software that `tokio-graceful` depends upon:

| | name | projects |
| - | - | - |
| 💌 | [Tokio](https://github.com/tokio-rs) | (Tokio, Async Runtime)
| 💌 | [Sean McArthur](https://github.com/seanmonstar) | (Tokio)