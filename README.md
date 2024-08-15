# Brush

## Overview

Brush is 3D reconstructiong engine using gaussian splatting, made to be highly portable, flexible and fast. It can both render and train on macOS/windows/linux, in a browser, and on Android. It uses the [Burn](https://github.com/tracel-ai/burn) framework, and custom WGSL kernels.

The default app (`crates/brush-desktop`) uses egui and can be used to visualize a pre-trained splat from a `.ply` file, or train on data given in a `.zip` file. The expected format is either the format used in the synthetic NeRF example data, or COLMAP data.

While training, training data and statistcs can be visualized with [rerun](https://rerun.io/). To install rerun, follow the [instructions](https://rerun.io/docs/getting-started/installing-viewer).

This project is a proof of concept - it currently only does basic training on 'vanilla' gaussian splats.

## Getting strated

Install rust 1.78+ and run `cargo run` or `cargo run --release`. You can run tests with `cargo test --workspace`. To run with `rerun` use `cargo run --features=rerun`.

For profiling the kernels you can use tracy

### Desktop

Simply `cargo run`. Windows uses Vulkan by default, but this can be changed in `crates/brus-viewer/wgpu_config.rs`. `DX12` works but seems to be much slower to compile all the kernels. macOS should be fully compatible and testd.

### Android

To build on Android, see the README instructions in crates/brush-android.

### Web

We use `trunk` to build for the web. Install trunk, and from the crates/brush-desktop folder run `trunk serve` to run a development server.

### iOs

Things *should* work on iOs but there is no project setup, and I haven't tested this currently.

## Benchmarks

Rendering performance is expected to be very competitive with eg. gsplat, training performance will likely still be slower. More detailled benchmarks will be added soon.

For profiling, you can use [tracy](https://github.com/wolfpld/tracy) and run with `cargo run --release --feature=tracy`. The UI will have some more options to sync the GPU so GPU times can be roughly measured.

## Example data

To get started, you can use some reference data taken from the [mipnerf](https://jonbarron.info/mipnerf360/) and [gaussian splatting](https://github.com/graphdeco-inria/gaussian-splatting) paper.

### Reference ply files
- [bicycle (1.4GB)](https://drive.google.com/file/d/1kHkNqGFLLutRt3R7k2tGkjGwfXnPLnCi/view?usp=sharing)
- [bonsai (300MB)](https://drive.google.com/file/d/1jf4bjaeTGeru1PQS_Ue716uc_edRbAPd/view?usp=sharing)
- [counter (290MB)](https://drive.google.com/file/d/1O89SIHcWdmrWi75Cf6tDrv2Dl6yGndcz/view?usp=sharing)
- [drjohnson (800MB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)
- [garden (1.3GB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)
- [kitchen (440MB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)
- [playroom (600MB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)
- [room (375MB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)
- [stump (1.15GB)](https://drive.google.com/file/d/13FEQ7UZHYwymBTwxzpPeJob4cr8VxUTV/view?usp=sharing)

### Synthetic nerf training data
- [Chair](https://drive.google.com/file/d/13Q6s0agTW1_a7cFGcSmll1-Aikq_OPKe/view?usp=sharing)
- [Drums](https://drive.google.com/file/d/1j8TuMiGb84YtlrZ0gnkMNOzUaIJqz0SY/view?usp=sharing)
- [Ficus](https://drive.google.com/file/d/1VzT5SDiBefn9fvRw7LeYjUfDBZHCyzQ4/view?usp=sharing)
- [Hotdog](https://drive.google.com/file/d/1hOjnCV8XdXClV2eC6c9H6PIQTUYv8zys/view?usp=sharing)
- [Lego](https://drive.google.com/file/d/1VxsNFTHhgxK9iCOgkuKxakBXJfgHUOQk/view?usp=sharing)
- [Materials](https://drive.google.com/file/d/1L7J5PNBcLcXde6CqzzkaNxHt7JtG2GIW/view?usp=sharing)
- [Mic](https://drive.google.com/file/d/1SA0NNi0HsUHE6FgAP8XpD23N1xftsrr-/view?usp=sharing)
- [Ship](https://drive.google.com/file/d/1rzL0KrWuLFebT1hLLm4uYnrNXNTkfjxM/view?usp=sharing)

## Tech

### Live training

Brush can render while training on another thread. This allows you to watch the training dynamics live! It could also be used as a helpful preview while capturing data.

On the web this runs in a seperate async task, instead of a thread, as threading on WASM doesn't seem to be viable.

### Kernels

The kernels are written in a "sparse" style, that is, only work for visible gaussians is done, though the final gradients are dense.

Brush uses a GPU radix sort based on [FidelityFX](https://www.amd.com/en/products/graphics/technologies/fidelityfx.html) (see `crates/brush-sort`). It splits the tile radix sort in two parts. First it only sorts by depth, then sorts by tile ID, which saves some sorting time compared to the original sort.

Compatibility with WebGPU does bring some challenges, even with (the excellent) [wgpu](https://github.com/gfx-rs/wgpu).
- WebGPU lacks native atomic floating point additions. Instead, gradients are scattered in the backwards rasterization to a buffer and later aggregated.
- WebGPU also lacks subgroup operations. wgpu recently gained the ability to use these on native platforms however, these should be integrated for a nice speedup.
- Lastly, GPU readbacks are tricky on WebGPU. The rendering pass cannot do this unless the whole rendering becomes async with its own perils. The reference tile renderer requires reading back the number of "intersections", but this is not feasible. This is worked around by assuming a worst case, and the rasterizer uses a technique to cull away more intersections by intersecting the gaussian ellipses with screenspace tiles.

The WGSL kernels use [naga_oil](https://github.com/bevyengine/naga_oil) to manage imports. brush-wgsl additionally does some reflection to generate rust code to send uniform data to a kernel. In the future, it might be possible to port the kernels to Burns new CubeCL system which would make this easier, and add CUDA compatibility.

Brush shares the same [wgpu](https://github.com/gfx-rs/wgpu) device for the UI (egui) and the training. There is no multi device training yet.

The reference gaussian splatting kernels rely on shared memory when rasterizing. Brush used to do this but this was benchmarked to be *slower* on an M1, so this is disabled at the moment. I hope to bring this back as it was
faster on desktop GPUs.

Gradients have been verified to match against a reference implementation for simple cases.

## Acknowledgements

**The Burn team**, for helping out with the tricky custom kernel integrations

**Raph Levien**, for the [original version](https://github.com/googlefonts/compute-shader-101/pull/31) of the GPU radix sort.

**Ben Poole**, for supporting the project.
