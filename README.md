# iBoardBot Web

An unofficial iBoardBot client that does not require you to use an
unauthenticated, unencrypted cloud solution :)

Instead, it communicates with the iBoardBot through serial, for example from a
Raspberry Pi.

Work in progress.

## Starting

This project requires Rust nightly.

To start the server:

    $ cargo run

## Fabric

Fabric was built with the following options:

    $ node build.js modules=interaction,text,no-svg-export
