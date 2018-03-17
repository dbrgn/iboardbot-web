# iBoardBot Web

An unofficial iBoardBot client that does not require you to use an
unauthenticated, unencrypted cloud solution :)

Instead, it communicates with the iBoardBot through serial, for example from a
Raspberry Pi.

This project requires the iBoardBot to load [my fork of the
firmware](https://github.com/dbrgn/iBoardbot).

This is what it looks like in the browser:

![screenshot](screenshot-small.png)

Work in progress.

## Starting

This project currently requires Rust nightly. The easiest way to get that is
through [rustup](https://rustup.rs/).

To start the server:

    $ cargo run -c config.json

The `-c` argument is optional, it defaults to `config.json`.

The configfile needs to look like this:

    {
        "device": "/dev/ttyACM0"
    }

If you use the original iBoardBot Arduino, then the device will probably be
`/dev/ttyACM0`.

Now the server is running on `http://127.0.0.1:8000/`.

## Fabric.js

Fabric (used for the preview in the frontend) was built with the following options:

    $ node build.js modules=interaction,text,no-svg-export

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT) at your option.
