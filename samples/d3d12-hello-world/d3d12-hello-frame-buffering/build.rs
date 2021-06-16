fn main() {
    println!("!cargo:rerun-if-changed=src/hello-frame-buffering-shaders.hlsl");
    std::fs::copy(
        "src/hello-frame-buffering-shaders.hlsl",
        std::env::var("OUT_DIR").unwrap() + "/../../../hello-frame-buffering-shaders.hlsl",
    )
    .expect("Copy");
}
