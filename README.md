# iBoardBot Web

An unofficial iBoardBot client that does not require you to use an
unauthenticated, unencrypted cloud solution :)

Instead, it communicates with the iBoardBot through serial, for example from a
Raspberry Pi.

This project requires the iBoardBot to load [my fork of the firmware](https://github.com/dbrgn/iBoardbot).

![screenshot](screenshot-small.png)

Work in progress.

## Starting

This project requires Rust nightly.

To start the server:

    $ cargo run /dev/<ttyACMx>

## Fabric

Fabric was built with the following options:

    $ node build.js modules=interaction,text,no-svg-export
