#!/bin/bash -e
cargo b --release --target i686-pc-windows-gnu
rm target/styxz.zip || true
7z a target/styxz.zip target/i686-pc-windows-gnu/release/styx_z.exe dist/run_proxy.bat Cargo.toml Cargo.lock LICENSE.md
7z a target/styxz.zip -r src
7z rn target/styxz.zip dist/run_proxy.bat run_proxy.bat src sources/src Cargo.toml sources/Cargo.toml Cargo.lock sources/Cargo.lock target/i686-pc-windows-gnu/release/styx_z.exe styx_z.exe
