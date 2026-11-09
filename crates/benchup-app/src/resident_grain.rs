//! Compact per-object grain axes. Preview motion updates textures, not face meshes.
use super::*;
const WIDTH: usize = 512;

#[derive(Default)]
pub(super) struct Cache {
    key: Option<Key>,
    axes: Arc<Vec<[f32; 4]>>,
    indices: Arc<Vec<u32>>,
    objects: Vec<(u64, u32)>,
    axes_texture: Arc<OnceLock<Texture>>,
    index_texture: Arc<OnceLock<Texture>>,
}
#[derive(PartialEq)]
struct Key {
    revision: u64,
    enabled: bool,
    preview: BTreeMap<u64, DVec3>,
}
pub(super) struct Data {
    axes: Arc<Vec<[f32; 4]>>,
    indices: Arc<Vec<u32>>,
    axes_texture: Arc<OnceLock<Texture>>,
    index_texture: Arc<OnceLock<Texture>>,
}
impl Cache {
    pub fn update(
        &mut self,
        model: &Model,
        hierarchy: &appearance::Hierarchy,
        key: &SceneKey,
    ) -> Data {
        let objects: Vec<_> = model
            .objects
            .keys()
            .map(|id| (*id, hierarchy.slot(*id)))
            .collect();
        if self.objects != objects || self.indices.is_empty() {
            let length = (objects.iter().map(|v| v.1 as usize).max().unwrap_or(0) + 1)
                .div_ceil(WIDTH)
                * WIDTH;
            let mut indices = vec![0u32; length];
            for (index, (_, slot)) in objects.iter().enumerate() {
                indices[*slot as usize] = index as u32 + 1;
            }
            self.indices = Arc::new(indices);
            self.index_texture = Arc::new(OnceLock::new());
            self.objects = objects;
            self.key = None;
        }
        let next = Key {
            revision: key.model_revision,
            enabled: key.display.grain_direction,
            preview: key.grain_preview.clone(),
        };
        if self.key.as_ref() != Some(&next) {
            let length = (self.objects.len() + 1).div_ceil(WIDTH) * WIDTH;
            if self.axes.len() != length {
                self.axes_texture = Arc::new(OnceLock::new());
            }
            let mut axes = vec![[0.0f32; 4]; length];
            for (index, (id, _)) in self.objects.iter().enumerate() {
                let mut ancestor = Some(*id);
                let mut assigned = None;
                let mut preview = None;
                while let Some(id) = ancestor {
                    if preview.is_none() {
                        preview = next.preview.get(&id).copied();
                    }
                    if assigned.is_none() && next.enabled {
                        assigned = model
                            .objects
                            .get(&id)
                            .and_then(|object| object.world_grain_axis());
                    }
                    ancestor = hierarchy.parent(id);
                }
                if let Some((axis, width)) = preview
                    .map(|axis| (axis, 1.15))
                    .or_else(|| assigned.map(|axis| (axis, 0.85)))
                {
                    axes[index + 1] = axis.extend(width).as_vec4().to_array();
                }
            }
            self.axes = Arc::new(axes);
            self.key = Some(next);
        }
        Data {
            axes: Arc::clone(&self.axes),
            indices: Arc::clone(&self.indices),
            axes_texture: Arc::clone(&self.axes_texture),
            index_texture: Arc::clone(&self.index_texture),
        }
    }
}
pub(super) struct Texture {
    texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pixel_bytes: u32,
    uploaded: Mutex<usize>,
}
impl Texture {
    fn new(
        device: &wgpu::Device,
        count: usize,
        format: wgpu::TextureFormat,
        pixel_bytes: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Resident grain axes"),
            size: wgpu::Extent3d {
                width: WIDTH as u32,
                height: (count / WIDTH) as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            pixel_bytes,
            uploaded: Mutex::new(0),
        }
    }
    fn update(&self, queue: &wgpu::Queue, bytes: &[u8], identity: usize) {
        let mut uploaded = self.uploaded.lock().unwrap();
        if *uploaded == identity {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH as u32 * self.pixel_bytes),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: WIDTH as u32,
                height: (bytes.len() / (WIDTH * self.pixel_bytes as usize)) as u32,
                depth_or_array_layers: 1,
            },
        );
        *uploaded = identity;
    }
}
impl Data {
    pub fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (&wgpu::TextureView, &wgpu::TextureView) {
        let axes = self.axes_texture.get_or_init(|| {
            Texture::new(
                device,
                self.axes.len(),
                wgpu::TextureFormat::Rgba32Float,
                16,
            )
        });
        let indices = self.index_texture.get_or_init(|| {
            Texture::new(device, self.indices.len(), wgpu::TextureFormat::R32Uint, 4)
        });
        axes.update(
            queue,
            bytemuck::cast_slice(self.axes.as_slice()),
            Arc::as_ptr(&self.axes) as usize,
        );
        indices.update(
            queue,
            bytemuck::cast_slice(self.indices.as_slice()),
            Arc::as_ptr(&self.indices) as usize,
        );
        (&axes.view, &indices.view)
    }
}
