# StyxZ

## Build Instructions
* Install Rust >= 1.64.0 (see https://www.rust-lang.org/tools/install - or  download https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe directly)
* Run `rustup target add i686-pc-windows-msvc`
* Install CLANG (Download https://github.com/llvm/llvm-project/releases/download/llvmorg-15.0.2/LLVM-15.0.2-win64.exe)
* Set the environment variable `LIBCLANG_PATH` to `C:\Program Files\LLVM\bin`
* Run `cargo build release --target i686-pc-windows-msvc` (in the sources folder)
* Find the executable in `target/i686-pc-windows-msvc/release/styx_z.exe`

