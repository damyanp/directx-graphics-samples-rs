use d3dx12::build::copy_data_file;

fn main() {
    copy_data_file("squidroom.bin");
    copy_data_file("src/rendering/multithreading-shaders.hlsl");
}
