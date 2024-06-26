use glam::{IVec3, Vec3};

use self::{storage::ChunkBlockStorage, visibility_graph::VisibilityGraph};
use super::position_types::{ChunkPosition, LocalBlockPosition};
use crate::{
    block::{BlockId, BLOCK_AIR},
    util::{
        size::{Size2, Size3},
        vector_map::VectorMapExt,
    },
};

pub mod side;
pub mod storage;
pub mod visibility_graph;

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_SIZE_2D: Size2 = Size2::splat(CHUNK_SIZE);
pub const CHUNK_SIZE_3D: Size3 = Size3::splat(CHUNK_SIZE);
pub const CHUNK_SIZE_LOG2: usize = 5;
pub const CHUNK_SIZE_SQUARED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_CUBED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_U32: u32 = CHUNK_SIZE as u32;
pub const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;
pub const CHUNK_SIZE_RECIP: f32 = 1.0 / (CHUNK_SIZE as f32);

#[derive(Clone, Debug)]
pub struct Chunk {
    pos: ChunkPosition,
    blocks: ChunkBlockStorage,
    visibility_graph: VisibilityGraph,
}

impl Chunk {
    pub fn new(pos: ChunkPosition, blocks: Vec<BlockId>) -> Self {
        // this function is called from a parallel thread so it's OK to perform intensive tasks
        // here
        let visibility_graph = VisibilityGraph::compute(&blocks);

        Self {
            pos,
            blocks: ChunkBlockStorage::new(blocks),
            visibility_graph,
        }
    }

    /// Returns the underlying block storage
    pub fn get_block_storage(&self) -> &ChunkBlockStorage {
        &self.blocks
    }

    /// Returns the block ID at the given position.
    /// Panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPosition) -> BlockId {
        self.blocks.get_block(pos)
    }

    /// Returns the block ID at the given position.
    /// Panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPosition, new_id: BlockId) {
        self.blocks.set_block(pos, new_id)
    }

    /// Returns this chunk's position
    pub fn position(&self) -> ChunkPosition {
        self.pos
    }

    /// Returns the computed visibility graph for this chunk
    pub fn visibility_graph(&self) -> VisibilityGraph {
        self.visibility_graph
    }

    /// Marches through the chunk along the ray with the given origin and direction, using the DDA
    /// algorithm
    /// If a block was hit, returns the position of that block in the chunk and face index of the
    /// hit face
    pub fn raymarch(
        &self,
        ray_origin: Vec3,
        ray_direction: Vec3,
        previous_chunk_pos: Option<ChunkPosition>,
        maximum_distance: f32,
    ) -> Option<ChunkHit> {
        pub const EPS: f32 = 1e-3;

        let dir_step = ray_direction.map(|component| if component >= 0.0 { 1.0 } else { 0.0 });
        let dir_recip = ray_direction.recip();

        let mut t = 0.0;
        let mut previous_block_pos: Option<LocalBlockPosition> = None;

        while t < maximum_distance {
            let ray_pos = ray_origin + ray_direction * t;
            let block_pos = ray_pos.floor().as_ivec3();

            if !Size3::splat(CHUNK_SIZE).contains_ivec3(block_pos) {
                // escaped chunk; no intersection
                return None;
            }

            let block_pos = LocalBlockPosition::from(block_pos.as_uvec3());
            if self.get_block(block_pos) != BLOCK_AIR {
                // hit a block
                return Some(ChunkHit {
                    local_hit_pos: block_pos,
                    hit_normal: previous_block_pos
                        .map(|previous_block_pos| {
                            previous_block_pos.as_ivec3() - block_pos.as_ivec3()
                        })
                        .or_else(|| {
                            previous_chunk_pos.map(|previous_chunk_pos| {
                                previous_chunk_pos.as_ivec3() - self.position().as_ivec3()
                            })
                        }),
                });
            }

            // advance to the next block position
            let deltas = (dir_step - ray_pos.fract_gl()) * dir_recip;
            t += deltas.min_element().max(EPS);

            previous_block_pos = Some(block_pos);
        }

        None
    }
}

/// Returned by `Chunk::raymarch` if a block was hit
pub struct ChunkHit {
    pub local_hit_pos: LocalBlockPosition,
    pub hit_normal: Option<IVec3>,
}
