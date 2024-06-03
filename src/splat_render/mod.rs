use crate::camera::Camera;
use burn::backend::Autodiff;
use burn::prelude::Int;
use burn::tensor::ops::FloatTensor;
use burn::{
    backend::wgpu::{AutoGraphicsApi, JitBackend, WgpuRuntime},
    tensor::{Shape, Tensor},
};
use burn_compute::server::Binding;
use burn_compute::{channel::ComputeChannel, client::ComputeClient, server::ComputeServer};
use burn_cube::Runtime;
use burn_jit::JitElement;
use burn_wgpu::{JitTensor, WgpuDevice};

mod dim_check;
mod kernels;
mod prefix_sum;
mod radix_sort;
mod shaders;

pub mod render;
pub mod sync_span;

pub type BurnBack = JitBackend<BurnRuntime, f32, i32>;
pub type BurnDiffBack = Autodiff<JitBackend<BurnRuntime, f32, i32>>;

pub type BurnRuntime = WgpuRuntime<AutoGraphicsApi>;
type BurnClient =
    ComputeClient<<BurnRuntime as Runtime>::Server, <BurnRuntime as Runtime>::Channel>;

#[derive(Debug, Clone)]
pub(crate) struct RenderAux<B: Backend> {
    pub num_visible: Tensor<B, 1, Int>,
    pub num_intersects: Tensor<B, 1, Int>,
    pub tile_bins: Tensor<B, 3, Int>,
    pub radii_compact: Tensor<B, 1>,
    pub depthsort_gid_from_isect: Tensor<B, 1, Int>,
    pub compact_from_depthsort_gid: Tensor<B, 1, Int>,
    pub depths: Tensor<B, 1>,
    pub cum_tiles_hit: Tensor<B, 1, Int>,
    pub conic_comps: Tensor<B, 2>,
    pub colors: Tensor<B, 2>,
    pub final_index: Tensor<B, 2, Int>,
    pub global_from_compact_gid: Tensor<B, 1, Int>,
    pub xys: Tensor<B, 2>,
}

/// We create our own Backend trait that extends the Burn backend trait.
pub trait Backend: burn::tensor::backend::Backend {
    // Render splats
    // Project splats processing step. This produces
    // a whole bunch of gradients that we store.
    // The return just happens to be the xy screenspace points
    // which we use to 'carry' the gradients'.
    fn render_gaussians(
        cam: &Camera,
        img_size: glam::UVec2,
        means: FloatTensor<Self, 2>,
        xy_dummy: FloatTensor<Self, 2>,
        log_scales: FloatTensor<Self, 2>,
        quats: FloatTensor<Self, 2>,
        colors: FloatTensor<Self, 2>,
        raw_opacity: FloatTensor<Self, 1>,
        background: glam::Vec3,
        render_u32_buffer: bool,
    ) -> (FloatTensor<Self, 3>, RenderAux<Self>);
}

// TODO: In rust 1.80 having a trait bound here on the inner backend would be great.
// For now all code using it will need to specify this bound itself.
pub trait AutodiffBackend: Backend + burn::tensor::backend::AutodiffBackend {}
impl AutodiffBackend for BurnDiffBack {}

// Reserve a buffer from the client for the given shape.
fn create_tensor<E: JitElement, const D: usize>(
    shape: [usize; D],
    device: &WgpuDevice,
    client: &BurnClient,
) -> JitTensor<BurnRuntime, E, D> {
    let shape = Shape::new(shape);
    let bufsize = shape.num_elements() * core::mem::size_of::<E>();
    let buffer = client.empty(bufsize);

    #[cfg(test)]
    {
        use burn::tensor::ops::FloatTensorOps;

        // for tests - make doubly sure we're not accidentally relying on values
        // being initialized to zero by adding in some random noise.
        let f =
            JitTensor::<BurnRuntime, f32, D>::new(client.clone(), device.clone(), shape, buffer);
        bitcast_tensor(BurnBack::float_add_scalar(f, -12345.0))
    }

    #[cfg(not(test))]
    JitTensor::new(client.clone(), device.clone(), shape, buffer)
}

// Convert a tensors type. This only reinterprets the data, and doesn't
// do any actual conversions.
fn bitcast_tensor<const D: usize, EIn: JitElement, EOut: JitElement>(
    tensor: JitTensor<BurnRuntime, EIn, D>,
) -> JitTensor<BurnRuntime, EOut, D> {
    JitTensor::new(tensor.client, tensor.device, tensor.shape, tensor.handle)
}

fn read_buffer_as_u32<S: ComputeServer, C: ComputeChannel<S>>(
    client: &ComputeClient<S, C>,
    binding: Binding<S>,
) -> Vec<u32> {
    let data = client.read(binding).read();
    data.chunks_exact(4)
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}

fn read_buffer_as_f32<S: ComputeServer, C: ComputeChannel<S>>(
    client: &ComputeClient<S, C>,
    binding: Binding<S>,
) -> Vec<f32> {
    let data = client.read(binding).read();
    data.chunks_exact(4)
        .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}
