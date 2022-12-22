use crate::Units;
use rsbwapi::*;

#[derive(Copy, Clone)]
pub struct Tile<T> {
    version: u32,
    item: T,
}

pub struct Grid<T, const N: usize> {
    version: u32,
    tiles: Box<[[Tile<T>; N]; N]>,
}

impl<T: Copy, const N: usize> Grid<T, N> {
    pub fn new(default: T) -> Self {
        let Ok(tiles) = vec![
                [Tile {
                    version: 0,
                    item: default
                }; N];
                N
            ]
            .try_into() else { unreachable!() };
        Self { version: 1, tiles }
    }

    pub fn reset(&mut self) {
        self.version += 1;
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        let tile = &self.tiles.get(y)?.get(x)?;
        if tile.version == self.version {
            Some(&tile.item)
        } else {
            None
        }
    }

    pub fn get_pos<const M: i32>(&self, pos: ScaledPosition<M>) -> Option<&T> {
        self.get(pos.x as usize, pos.y as usize)
    }

    pub fn set(&mut self, x: usize, y: usize, item: T) {
        self.tiles[y][x] = Tile {
            version: self.version,
            item,
        };
    }

    pub fn set_pos<const M: i32>(&mut self, pos: ScaledPosition<M>, item: T) {
        self.set(pos.x as usize, pos.y as usize, item);
    }

    pub fn modify_in_range(
        &mut self,
        x: usize,
        y: usize,
        range: usize,
        modifier: impl Fn(Option<T>, usize, usize) -> T,
    ) {
        let left = x.saturating_sub(range);
        let top = y.saturating_sub(range);
        let right = N.min(x + range + 1);
        let bottom = N.min(y + range + 1);
        let range_sq = range * range;
        for j in top..bottom {
            let dy = y.wrapping_sub(j);
            let dy_sq = dy.wrapping_mul(dy);
            for i in left..right {
                let dx = x.wrapping_sub(i);
                let dx_sq = dx.wrapping_mul(dx);
                if dx_sq + dy_sq <= range_sq {
                    let tile = &mut self.tiles[j][i];
                    tile.version = self.version;
                    tile.item = modifier(
                        if tile.version == self.version {
                            Some(tile.item)
                        } else {
                            None
                        },
                        i,
                        j,
                    );
                }
            }
        }
    }
}

pub struct Grids {
    // pub ground_threat: Grid<u16, 256>,
    // pub air_threat: Grid<u16, 256>,
    pub unit_walkability: Grid<UnitId, { 256 * 8 }>,
}

impl Grids {
    pub fn new() -> Self {
        Self {
            // ground_threat: Grid::new(0),
            // air_threat: Grid::new(0),
            unit_walkability: Grid::new(UnitId::MAX),
        }
    }

    pub fn get_occupant(&self, pos: WalkPosition) -> Option<UnitId> {
        self.unit_walkability.get_pos(pos).copied()
    }

    pub fn update(&mut self, units: &Units) {
        // self.ground_threat.reset();
        // self.air_threat.reset();
        self.unit_walkability.reset();

        for u in units.all().filter(|u| !u.flying()) {
            let dim = u.dimensions();
            for wp in Rectangle::new(
                dim.tl.to_walk_position(),
                (dim.br + (7, 7)).to_walk_position(),
            ) {
                self.unit_walkability.set_pos(wp, u.id());
            }
        }
        // threat maps
        // for e in &units.enemy {
        //     let (x, y) = e.tile_position().into();
        //     let range = (e.get_ground_weapon().max_range + 31) / 32;
        //     if e.get_type().ground_weapon() != WeaponType::None {
        //         self.ground_threat.modify_in_range(
        //             x as usize,
        //             y as usize,
        //             range as usize,
        //             |i, _, _| i.unwrap_or(0) + 1,
        //         );
        //     }
        //     if e.get_type().air_weapon() != WeaponType::None {
        //         self.air_threat.modify_in_range(
        //             x as usize,
        //             y as usize,
        //             range as usize,
        //             |i, _, _| i.unwrap_or(0) + 1,
        //         );
        //     }
        // }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_range() {
        let mut grid = Grid::<_, 100>::new(false);
        grid.modify_in_range(10, 10, 20, |_, _, _| true);

        assert_eq!(grid.get(0, 0), Some(&true));
        assert_eq!(grid.get(29, 29), None);
        assert_eq!(grid.get(30, 10), Some(&true));
        assert_eq!(grid.get(31, 10), None);
    }
}
