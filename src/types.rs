pub(crate) type Bb = ((i64, i64), (i64, i64));

///index into the vec of *total tiles*
///
///created while matching
#[repr(C)]
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, bytemuck::Pod, bytemuck::Zeroable,
)]
pub(crate) struct MatchIndex(pub(crate) i32);

///index into a dense array containing only tiles *used in the mosaic*
///
///created for the gpu-side palette
///
///for dynamic mosaics this is the same as the MatchIndex
#[repr(C)]
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, bytemuck::Pod, bytemuck::Zeroable,
)]
pub(crate) struct DenseIndex(pub(crate) i32);

///index into a dense array containing *every frame* for each tile used in the mosaic (each tile can consist of N frames)
///
///created for the cpu-side streaming buffers
///
///for static mosaics with static tiles, this is the same as the DenseIndex
///
///for dynamic mosaics, this is the same as both the MatchIndex and DenseIndex
#[repr(C)]
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, bytemuck::Pod, bytemuck::Zeroable,
)]
pub(crate) struct StoreIndex(pub(crate) i32);
