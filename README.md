# FirefoxRouter

A Windows utility that acts as a default browser proxy for Firefox. When you have multiple Firefox profiles, clicking a link in another application always opens it in the default profile even if you're actively using a different one. FirefoxRouter detects which Firefox profile is currently running and routes the URL to that profile instead.

## Usage

Register FirefoxRouter as your default browser:

```sh
FirefoxRouter.exe --register
```

After registering, set it as the default browser in Windows Settings > Default Apps. Incoming URLs and HTML files will now open in whichever Firefox profile is currently active.

To unregister:

```sh
FirefoxRouter.exe --unregister
```

## Building

Requires the [Rust toolchain](https://rustup.rs/).

```sh
cargo build --release
```

The binary will be at `target/release/FirefoxRouter.exe`.

## License

[MIT](LICENSE)
