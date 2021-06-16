fn main() {
    println!("!cargo:rerun-if-changed=src/hello-texture-shaders.hlsl");
    std::fs::copy(
        "src/hello-texture-shaders.hlsl",
        std::env::var("OUT_DIR").unwrap() + "/../../../hello-texture-shaders.hlsl",
    )
    .expect("Copy");
}
