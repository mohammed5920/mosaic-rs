use std::sync::atomic::{AtomicI32, Ordering};

use kiddo::{ImmutableKdTree, dist::SquaredEuclidean};

use crate::tiles::Tile;

pub struct Matchmaker {
    kiddie: ImmutableKdTree<u8, 3>,
    ///with capacity 16_777_216
    colour_map: Vec<AtomicI32>,
}

impl Matchmaker {
    pub fn from_tiles(tiles: &[Tile]) -> Matchmaker {
        let owned_references: Vec<_> = tiles.iter().map(|t| t.average_colour()).collect();
        let mut colour_map = Vec::with_capacity(16_777_216);
        colour_map.extend((0..16_777_216).map(|_| AtomicI32::new(-1)));
        Matchmaker {
            kiddie: ImmutableKdTree::new_from_slice(&owned_references)
                .expect("Max no. of tiles is 16_777_216"),
            colour_map,
        }
    }

    pub fn matchmake(&self, query: &[[u8; 3]]) -> Vec<i32> {
        query
            .iter()
            .map(|pixel| {
                let [r, g, b] = *pixel;
                let key = (r as usize) << 16 | (g as usize) << 8 | (b as usize);
                let mut res = self.colour_map[key].load(Ordering::Relaxed);
                if res == -1 {
                    res = self
                        .kiddie
                        .query(pixel)
                        .nearest_one::<SquaredEuclidean<f64>>()
                        .execute()
                        .item as i32;
                    self.colour_map[key].store(res, Ordering::Relaxed);
                }
                res
            })
            .collect()
    }
}
