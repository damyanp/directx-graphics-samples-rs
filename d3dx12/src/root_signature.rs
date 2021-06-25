#![allow(non_snake_case)]

use bindings::Windows::Win32::Graphics::Direct3D12::*;
use std::{intrinsics::transmute, marker::PhantomData, ops::Deref};

#[repr(C)]
#[derive(:: std :: clone :: Clone, :: std :: marker :: Copy)]
pub struct D3D12_ROOT_PARAMETER1<'a> {
    pub ParameterType: D3D12_ROOT_PARAMETER_TYPE,
    pub Anonymous: D3D12_ROOT_PARAMETER1_0<'a>,
    pub ShaderVisibility: D3D12_SHADER_VISIBILITY,
}

#[repr(C)]
#[derive(:: std :: clone :: Clone, :: std :: marker :: Copy)]
pub union D3D12_ROOT_PARAMETER1_0<'a> {
    pub DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1<'a>,
    pub Constants: D3D12_ROOT_CONSTANTS,
    pub Descriptor: D3D12_ROOT_DESCRIPTOR1,
}

impl<'a> Deref for D3D12_ROOT_PARAMETER1<'a> {
    type Target = bindings::Windows::Win32::Graphics::Direct3D12::D3D12_ROOT_PARAMETER1;

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(self) }
    }
}

#[repr(C)]
#[derive(:: std :: clone :: Clone, :: std :: marker :: Copy)]
pub struct D3D12_ROOT_DESCRIPTOR_TABLE1<'a> {
    pub NumDescriptorRanges: u32,
    pub pDescriptorRanges: *const D3D12_DESCRIPTOR_RANGE1,
    pub phantom: PhantomData<&'a [D3D12_DESCRIPTOR_RANGE1]>,
}

pub fn descriptor_table<'a>(
    ranges: &'a [D3D12_DESCRIPTOR_RANGE1],
    visibility: D3D12_SHADER_VISIBILITY,
) -> D3D12_ROOT_PARAMETER1 {
    D3D12_ROOT_PARAMETER1 {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
        ShaderVisibility: visibility,
        Anonymous: D3D12_ROOT_PARAMETER1_0 {
            DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 {
                NumDescriptorRanges: ranges.len() as u32,
                // TODO: transmute required here because pDescriptorRanges is incorrectly marked mutable
                pDescriptorRanges: unsafe { transmute(ranges.as_ptr()) },
                phantom: PhantomData,
            },
        },
    }
}
