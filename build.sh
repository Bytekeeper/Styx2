#!/bin/bash -e

#BINDGEN_EXTRA_CLANG_ARGS="--target=`llvm-config --host-target`" cargo build --target=i686-pc-windows-gnu --release
#BINDGEN_EXTRA_CLANG_ARGS="--target=x86_64-pc-linux-gnu" cargo build --target=i686-pc-windows-gnu --release
# cargo build --features cvis && cp target/i686-pc-windows-gnu/debug/styx_z.exe ~/.scbw/bots/styx_z/AI/
cargo build --features cvis --release && cp target/i686-pc-windows-gnu/release/styx_z.exe ~/.scbw/bots/styx_z/AI/
#cargo build && cp target/i686-pc-windows-gnu/debug/styx_z.exe ~/.scbw/bots/styx_z/AI/
# cp target/i686-pc-windows-gnu/release/styx_z.exe ~/.scbw/bots/styx_z/AI/

