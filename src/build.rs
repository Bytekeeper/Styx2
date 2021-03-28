use crate::*;

pub struct Build {
    builder_id: Lock<usize>,
    build_pos: Lock<TilePosition>,
    unit_type: UnitType,
    done: bool,
}

impl Build {
    pub fn new(unit_type: UnitType) -> Build {
        Build {
            builder_id: Lock::default(),
            build_pos: Lock::default(),
            unit_type,
            done: false,
        }
    }
    pub fn start_build(&mut self, frame: &mut Frame) -> NodeStatus {
        let self_ = frame.game.self_().unwrap();
        let game = frame.game;
        if self.done {
            return NodeStatus::Success;
        }
        if frame.available_gms < self.unit_type.price() {
            return NodeStatus::Running;
        }
        frame.available_gms -= self.unit_type.price();
        if self
            .builder_id
            .locked()
            .map(|id| game.get_unit(id))
            .flatten()
            .map(|u| u.get_type())
            == Some(self.unit_type)
        {
            self.done = true;
            return NodeStatus::Success;
        }
        let my_units = &frame.my_units;
        let base = my_units.iter().find(|u| u.get_type().is_resource_depot());
        if let Some(base) = base {
            let builder = self.builder_id.locked().map(|i| game.get_unit(i)).flatten();
            let unit_type = self.unit_type;
            let locked_position = self.build_pos.lock(
                || {
                    Rectangle::<TilePosition>::new(
                        base.get_tile_position() - (5, 5),
                        base.get_tile_position() + (5, 5),
                    )
                    .into_iter()
                    .min_by_key(|p| {
                        if game
                            .can_build_here(builder.as_ref(), *p, unit_type, true)
                            .unwrap_or(false)
                        {
                            p.distance_squared(base.get_tile_position())
                        } else {
                            u32::MAX
                        }
                    })
                },
                |pos| {
                    game.can_build_here(builder.as_ref(), *pos, unit_type, true)
                        .unwrap_or(false)
                },
            );
            if locked_position {
                self.builder_id.lock(
                    || {
                        frame
                            .available_units
                            .iter()
                            .filter(|u| u.get_type().is_worker())
                            .min_by_key(|u| {
                                u.get_tile_position()
                                    .distance_squared(base.get_tile_position())
                            })
                            .map(|u| u.get_id())
                    },
                    |i| {
                        game.get_unit(*i)
                            .map(|u| u.get_type().is_worker())
                            .unwrap_or(false)
                    },
                );
                let builder = self.builder_id.locked().map(|i| game.get_unit(i)).flatten();
                //println!("{:?} {:?}", pos_to_build, builder.map(|b| b.get_id()));
                if let Some(builder) = builder {
                    builder
                        .build(self.unit_type, self.build_pos.locked().unwrap())
                        .ok();
                    frame.available_units.swap_remove(
                        frame
                            .available_units
                            .iter()
                            .position(|u| u == &builder)
                            .expect("Builder to be available"),
                    );
                }
            }
        }
        NodeStatus::Running
    }
}
