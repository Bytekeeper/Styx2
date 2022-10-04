use crate::MyModule;
use ahash::AHashMap;
use rsbwapi::Game;
use rsbwapi::TilePosition;

pub struct SBase {
    pub position: TilePosition,
    pub last_explored: i32,
    pub starting_location: bool,
}

#[derive(Default)]
pub struct Bases {
    all: AHashMap<TilePosition, SBase>,
}

impl Bases {
    pub fn new(module: &MyModule) -> Self {
        let all: AHashMap<_, _> = module
            .map
            .bases
            .iter()
            .map(|b| {
                (
                    b.position,
                    SBase {
                        position: b.position,
                        last_explored: -1,
                        starting_location: module
                            .game
                            .get_start_locations()
                            .iter()
                            .any(|l| l.distance_squared(b.position) < 5 * 5),
                    },
                )
            })
            .collect();
        let mut result = Self { all };
        result.update(&module.game);
        result
    }

    pub fn all(&self) -> Vec<&SBase> {
        self.all.values().collect()
    }

    pub fn update(&mut self, game: &Game) {
        for base in self.all.values_mut() {
            if game.is_visible(base.position) {
                base.last_explored = game.get_frame_count();
            }
        }
    }
}
