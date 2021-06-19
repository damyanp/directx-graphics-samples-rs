use bindings::Windows::Win32::{Foundation::PSTR, Graphics::{Direct3D12::*, Dxgi::*}};

pub const DATA_FILE_NAME: &str = "SquidRoom.bin";

macro_rules! input_element_desc {
    { $( { $name:literal, $semantic_index:expr, $format:expr, $slot:expr, $offset:expr, $class:expr, $rate:expr } ),* }
    => { [
        $( D3D12_INPUT_ELEMENT_DESC{
            SemanticName: PSTR(concat!($name, "\0").as_ptr() as _),
            SemanticIndex: $semantic_index,
            Format: $format,
            InputSlot: $slot,
            AlignedByteOffset: $offset,
            InputSlotClass: $class,
            InstanceDataStepRate: $rate
        }
        ),* ]
    };
}

pub const STANDARD_VERTEX_DESCRIPTION : [D3D12_INPUT_ELEMENT_DESC; 4] = input_element_desc! {
    { "POSITION", 0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 0,  D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0 },
    { "NORMAL",   0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 12, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0 },
    { "TEXCOORD", 0, DXGI_FORMAT_R32G32_FLOAT,    0, 24, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0 },
    { "TANGENT",  0, DXGI_FORMAT_R32G32B32_FLOAT, 0, 32, D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, 0 }
};
