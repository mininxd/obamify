use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};
use crate::app::calculate::util::{GenerationSettings, get_images};
use crate::app::preset::UnprocessedPreset;
use std::borrow::Cow;
use pollster;
use wasm_bindgen_futures;

#[wasm_bindgen]
pub async fn generate_with_gpu(source_image: JsValue, target_image: JsValue, settings: JsValue, gpu: bool) -> Result<JsValue, JsValue> {
    let source: UnprocessedPreset = serde_wasm_bindgen::from_value(source_image).unwrap();
    let target: UnprocessedPreset = serde_wasm_bindgen::from_value(target_image).unwrap();
    let settings: GenerationSettings = serde_wasm_bindgen::from_value(settings).unwrap();

    let headless = Headless::new(gpu).await.ok_or_else(|| JsValue::from_str("Failed to initialize WebGPU"))?;
    let result = headless.generate(source, target, settings).await.map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_wasm_bindgen::to_value(&result).unwrap())
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SeedPos {
    xy: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SeedColor {
    rgba: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParamsCommon {
    width: u32,
    height: u32,
    n_seeds: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParamsJfa {
    width: u32,
    height: u32,
    step: u32,
    _pad: u32,
}

pub struct Headless {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Headless {
    pub async fn new(gpu: bool) -> Option<Self> {
        let backends = if gpu {
            wgpu::Backends::PRIMARY
        } else {
            wgpu::Backends::all()
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await;
        let (device, queue) = adapter?
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .ok()?;
        Some(Self { device, queue })
    }

    pub async fn generate(
        &self,
        unprocessed: UnprocessedPreset,
        target: UnprocessedPreset,
        settings: GenerationSettings,
    ) -> Result<Vec<u8>, String> {
        let source_img = image::ImageBuffer::from_vec(
            unprocessed.width,
            unprocessed.height,
            unprocessed.source_img.clone(),
        )
        .unwrap();
        let target_img = image::ImageBuffer::from_vec(
            target.width,
            target.height,
            target.source_img.clone(),
        )
        .unwrap();
        let (source_pixels, _, _) = get_images(source_img, target_img, &settings).unwrap();
        let seeds: Vec<SeedPos> = source_pixels.iter().enumerate().map(|(i, _)| {
            let x = (i as u32 % settings.sidelen) as f32;
            let y = (i as u32 / settings.sidelen) as f32;
            SeedPos { xy: [x, y] }
        }).collect();
        let colors: Vec<SeedColor> = source_pixels.iter().map(|(r, g, b)| {
            SeedColor { rgba: [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0, 1.0] }
        }).collect();

        // Buffers
        let seed_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("seeds"),
            contents: bytemuck::cast_slice(&seeds),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let color_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("colors"),
            contents: bytemuck::cast_slice(&colors),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let params_common_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params_common"),
            contents: bytemuck::bytes_of(&ParamsCommon {
                width: settings.sidelen,
                height: settings.sidelen,
                n_seeds: seeds.len() as u32,
                _pad: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let params_jfa_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params_jfa"),
            contents: bytemuck::bytes_of(&ParamsJfa {
                width: settings.sidelen,
                height: settings.sidelen,
                step: 1,
                _pad: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output_buffer"),
            size: (settings.sidelen * settings.sidelen * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let download_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("download_buffer"),
            size: (settings.sidelen * settings.sidelen * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Textures
        let ids_a_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ids_a"),
            size: wgpu::Extent3d {
                width: settings.sidelen,
                height: settings.sidelen,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let ids_b_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ids_b"),
            size: wgpu::Extent3d {
                width: settings.sidelen,
                height: settings.sidelen,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        // ... The rest of the implementation will go here

        Ok(vec![])
    }
}
