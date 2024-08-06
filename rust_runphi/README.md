## To install rust 
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
## install crosscompiler
apt install gcc-aarch64-linux-gnu 
## To build
cargo build --release --target=aarch64-unknown-linux-gnu
 
