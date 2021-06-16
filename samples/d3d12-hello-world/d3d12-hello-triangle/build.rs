fn main() {
    println!("!cargo:rerun-if-changed=src/hello-triangle-shaders.hlsl");
    std::fs::copy(
        "src/hello-triangle-shaders.hlsl",
        std::env::var("OUT_DIR").unwrap() + "/../../../hello-triangle-shaders.hlsl",
    )
    .expect("Copy");
}
