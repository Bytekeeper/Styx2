#[derive(Copy, Clone)]
pub struct Tile<T> {
    version: u32,
    item: T,
}

pub struct Grid<T, const N: usize> {
    version: u32,
    tiles: [[Tile<T>; N]; N],
}

impl<T: Copy, const N: usize> Grid<T, N> {
    pub fn new(default: T) -> Self {
        Self {
            version: 0,
            tiles: [[Tile {
                version: 0,
                item: default,
            }; N]; N],
        }
    }

    pub fn reset(&mut self) {
        self.version += 1;
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        let tile = &self.tiles[y][x];
        if tile.version == self.version {
            Some(&tile.item)
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, item: T) {
        self.tiles[y][x] = Tile {
            version: self.version,
            item,
        };
    }
}
