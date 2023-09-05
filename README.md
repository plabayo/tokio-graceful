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

Shutdown management for graceful shutdown of [tokio](https://tokio.rs/) applications.
Guard creating and usage is lock-free and the crate only locks when:

- the shutdown signal was not yet given and you wait with a (weak or strong) guard
  on whether or not it was in fact cancelled;
- the check of whether or not the app can shut down typically is locked until
  the shutdown signal was received and all (strong) guards were dropped.

This crate is written in 100% safe Rust code.

## Index

- [Examples](#examples): quick overview of how to use this crate;
    - Make sure to also check out the
      [Tokio TCP](https://github.com/plabayo/tokio-graceful/tree/main/examples/tokio_tcp.rs)
      and [Hyper](https://github.com/plabayo/tokio-graceful/tree/main/examples/hyper.rs) examples for typical "real world" usage!
- [Contributing information](#contributing) and special [shoutouts](#shoutouts).
- [Licensing](#license) info and what happens to [your contributions](#contribution).
- [Frequently Asked Questions](#faq)

## Examples

One example to show it all:

```rust
use std::time::Duration;
use tokio_graceful::Shutdown;

#[tokio::main]
async fn main() {
    // most users can just use `Shutdown::default()` to initiate
    // shutdown upon either Sigterm or CTRL+C (Sigkill).
    let signal = tokio::time::sleep(std::time::Duration::from_millis(100));
    let shutdown = Shutdown::new(signal);

    // you can use shutdown to spawn tasks that will
    // include a guard to prevent the shutdown from completing
    // aslong as these tasks are open
    shutdown.spawn_task(async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    });
    // or spawn a function such that you have access to the guard coupled to the task
    shutdown.spawn_task_fn(|guard| async move {
        let guard2 = guard.clone();
        guard.cancelled().await;
    });

    // this guard isn't dropped, but as it's a weak guard
    // it has no impact on the ref count of the common tokens.
    let guard_weak = shutdown.guard_weak();

    // this guard needs to be dropped as otherwise the shutdown is prevented;
    let guard = shutdown.guard();
    drop(guard);

    // guards can be downgraded to weak guards, to not have it be counted any longer in the ref count
    let weak_guard_2 = shutdown.guard().downgrade();

    // guards (weak or not) are cancel safe
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {},
        _ = weak_guard_2.into_cancelled() => {},
    }

    // you can also wait to shut down without any timeout limit
    // `shutdown.shutdown().await;`
    shutdown
        .shutdown_with_limit(Duration::from_secs(60))
        .await
        .unwrap();

    // once a shutdown is triggered the ::cancelled() fn will immediately return true,
    // forever, not just once. Even after shutdown process is completely finished.
    guard_weak.cancelled().await;

    // weak guards can be upgraded to regular guards to take into account for ref count
    let guard = guard_weak.upgrade();
    // even this one however will know it was cancelled
    guard.cancelled().await;
}
```

### Runnable Examples

While the above example shows pretty much all parts of this crate at once,
it might be useful to see examples on how this crate is to be used in
an actual production-like setting. That's what these runnable examples are for.

The runnable examples are best run with `RUST_LOG=trace` environment variable set,
such that you see the verbose logs of `tokio-graceful` and really see it in action
and get a sense on how it works, or at least its flow.

> [examples/tokio_tcp.rs](https://github.com/plabayo/tokio-graceful/tree/main/examples/tokio_tcp.rs)
>
> ```bash
> RUST_LOG=trace cargo run --example tokio_tcp
> ```

The `tokio_tcp` example showcases the original use case of why `tokio-graceful` shutdown was developed,
as it makes managing graceful shutdown from start to finish a lot easier, without immediately grabbing
to big power tools or providing more than is needed.

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

🎈 Thanks for your help improving the project! We are so happy to have
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

## FAQ

> What is the difference with <https://tokio.rs/tokio/topics/shutdown>?

<https://tokio.rs/tokio/topics/shutdown> is an excellent tutorial by the Tokio developers.
It is meant to teach and inspire you on how to be able to gracefully shutdown your
Tokio-driven application and also to give you a rough idea on when to use it.

That said, nothing stops you from applying what you learn in that tutorial directly
in your production application. It will work and very well so. However
there is a lot of management of components you have to do yourself.

> Ok, but what about the other crates on <https://crates.io/> that provide graceful shutdown?

They work fine and they are just as easy to use as this crate. However we think that
those crates offer more features then you need in a typical use case, are as a consequence
more complex on the surface as well as the machinery inside.

> How do I trigger the Shutdown from within a task?

You can achieve this by providing your own mechanism that you feed as the "signal"
to [`Shutdown::new`](https://docs.rs/tokio-graceful/0.1.0/tokio_graceful/struct.Shutdown.html#method.new). E.g. you could easily achieve this by using <https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html> so you can notify from any task where you wish and have your signal be
<https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html#method.notified>.

This is however not a usecase we have, as most web services (be it servers or proxies) typically
wish to run all its connections independent without critical failures. In such
environments there is no need for top-down cancellation mechanisms. Therefore we have
nothing built in as this allows us to keep the API and source code simpler, and on top of
that gives us the freedom to change some internal details in the future without having
to continue to support this usecase.
