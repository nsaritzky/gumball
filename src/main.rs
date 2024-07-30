fn main() {
    #[cfg(target_arch = "wasm32")]
    gumball::wasm_main();
    #[cfg(not(target_arch = "wasm32"))]
    gumball::native_main();
}
