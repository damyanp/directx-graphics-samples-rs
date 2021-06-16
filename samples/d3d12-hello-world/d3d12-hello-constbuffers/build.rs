fn main() {
    println!("!cargo:rerun-if-changed=src/hello-constbuffers-shaders.hlsl");
    std::fs::copy(
        "src/hello-constbuffers-shaders.hlsl",
        std::env::var("OUT_DIR").unwrap() + "/../../../hello-constbuffers-shaders.hlsl",
    )
    .expect("Copy");
}
