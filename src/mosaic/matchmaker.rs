use std::sync::atomic::{AtomicI32, Ordering};

use kiddo::{ImmutableKdTree, dist::SquaredEuclidean};

use crate::{mosaic::tiles::Tile, types::SparseIndex, util::colour_to_key};

pub(crate) struct MatchMaker {
    kiddie: ImmutableKdTree<u8, 3>,
    ///with capacity 16_777_216
    colour_map: Vec<AtomicI32>,
}

impl MatchMaker {
    pub(crate) fn new(tiles: &[Tile]) -> MatchMaker {
        assert!(
            tiles.len() <= 16_777_216,
            "max no. of tiles is 16_777_216 (RGB colour space)"
        );
        let mut colour_map = Vec::with_capacity(16_777_216);
        colour_map.extend((0..16_777_216).map(|_| AtomicI32::new(-1)));
        MatchMaker {
            kiddie: ImmutableKdTree::new_from_slice(
                &tiles.iter().map(|t| t.average_colour()).collect::<Vec<_>>(),
            )
            .expect("could not build kiddo tree"),
            colour_map,
        }
    }

    pub(crate) fn matchmake(&self, query: &[u8]) -> Vec<SparseIndex> {
        debug_assert!(
            query.len().is_multiple_of(3),
            "pixel array is not divisible by 3 (not valid RGB)"
        );
        query
            .as_chunks::<3>()
            .0
            .iter()
            .map(|pixel| {
                let key = colour_to_key(*pixel);
                let mut res = self.colour_map[key].load(Ordering::Relaxed);
                if res == -1 {
                    res = self
                        .kiddie
                        .query(pixel)
                        .nearest_one::<SquaredEuclidean<f32>>()
                        .execute()
                        .item as i32;
                    self.colour_map[key].store(res, Ordering::Relaxed);
                }
                res
            })
            .map(SparseIndex)
            .collect()
    }
}
