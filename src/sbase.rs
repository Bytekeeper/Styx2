use crate::{MyModule, SPlayer, SUnit};
use ahash::AHashMap;
use rsbwapi::Game;
use rsbwapi::TilePosition;

pub struct SBase {
    pub position: TilePosition,
    pub last_explored: i32,
    pub starting_location: bool,
    pub elevation_level: i32,
    pub player: Option<SPlayer>,
    pub resource_depot: Option<SUnit>,
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
                let resource_depot = module
                    .units
                    .all()
                    .filter(|it| {
                        it.get_type().is_resource_depot()
                            && it.tile_position().distance_squared(b.position) < 100
                    })
                    .next()
                    .cloned();
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
                        elevation_level: module.game.get_ground_height(b.position),
                        player: resource_depot.as_ref().map(|it| it.player()),
                        resource_depot,
                    },
                )
            })
            .collect();
        let mut result = Self { all };
        result.update(&module.game);
        result
    }

    pub fn all(&self) -> impl Iterator<Item = &SBase> {
        self.all.values()
    }

    pub fn update(&mut self, game: &Game) {
        for base in self.all.values_mut() {
            if game.is_visible(base.position) {
                base.last_explored = game.get_frame_count();
            }
        }
    }
}
