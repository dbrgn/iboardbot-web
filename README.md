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

## Building

Build debug build:

    $ cargo build

Build release build for Raspberry Pi:

    $ cargo build --release --target arm-unknown-linux-gnueabihf

## Starting

This project currently requires Rust 1.26+. The easiest way to get that is
through [rustup](https://rustup.rs/).

To start the server:

    $ cargo run -c config.json

The `-c` argument is optional, it defaults to `config.json`.

The configfile needs to look like this:

    {
        "device": "/dev/ttyACM0",
        "svg_dir": "/path/to/svgdir",
        "static_dir": "/srv/www/static",
        "interval_seconds": 900
    }

...or for preview-only:

    {
        "static_dir": "/srv/www/static"
    }

(Note: The `static_dir` key is optional, if left out it will use "static" relative to the CWD.)

If you use the original iBoardBot Arduino via USB, then the `device` will
probably be `/dev/ttyACM0`. The `svg_dir` points to the directory where SVG
files are stored for printing. And the `interval_seconds` value will determine
in which interval to start draws.

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
