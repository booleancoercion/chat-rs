# MPSC channels for Iced

Allows you to create mpsc channels whose endpoints feed into `iced` messages. Might not scale well at the moment, but it does at least work. Note that you may have to clone the repo and change `iced`'s git revision in `Cargo.toml`.

## Example
```sh
cargo run --example button
```
