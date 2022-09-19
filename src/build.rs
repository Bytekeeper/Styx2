use crate::*;

pub struct BuildParam {
    unit_type: UnitType,
    at: Option<TilePosition>,
}

impl BuildParam {
    pub fn build(unit_type: UnitType) -> Self {
        Self {
            unit_type,
            at: None,
        }
    }

    pub fn at(mut self, at: TilePosition) -> Self {
        self.at = Some(at);
        self
    }
}

impl MyModule {
    pub fn do_extractor_trick(&mut self) -> Result<(), FailureReason> {
        if self.tracker.available_gms.supply < 0 {
            return Err(FailureReason::misc(
                "Will need more than an extractor trick",
            ));
        }
        if self.tracker.available_gms.supply > 0 {
            return self.start_train(TrainParam::train(UnitType::Zerg_Drone));
        }

        if self
            .units
            .mine_all
            .iter()
            .any(|u| u.build_type().is_worker() && !u.completed())
        {
            for incomplete_refinery in self
                .units
                .mine_all
                .iter()
                .filter(|e| e.get_type().is_refinery() && !e.completed())
            {
                incomplete_refinery
                    .cancel_morph()
                    .map_err(|e| FailureReason::Bwapi(e));
            }
            // Done
        } else if !self
            .units
            .mine_all
            .iter()
            .any(|u| u.get_type().is_refinery())
        {
            let mut price = UnitType::Zerg_Extractor.price() + UnitType::Zerg_Drone.price();
            price.supply = 0;

            if self.tracker.available_gms > price {
                self.start_build(BuildParam::build(UnitType::Zerg_Extractor))?;
                // We can't start training yet, but we need to save the minerals
                self.tracker
                    .available_gms
                    .checked_sub(UnitType::Zerg_Drone.price());
            } else {
                self.tracker.available_gms.checked_sub(price);
            }
        }
        Ok(())
    }

    pub fn ensure_building_count(
        &mut self,
        building_type: UnitType,
        amount: usize,
    ) -> Result<(), FailureReason> {
        let mut amount = amount.saturating_sub(
            self.units
                .mine_all
                .iter()
                .filter(|u| u.get_type().is_successor_of(building_type))
                .count(),
        );
        // Worker might still be on its way, but will start the build - so reserve the money
        let pending = self
            .units
            .my_completed
            .iter()
            .filter(|u| {
                u.get_type() != building_type && u.build_type().is_successor_of(building_type)
            })
            .count();
        for _ in 0..pending {
            self.tracker
                .available_gms
                .checked_sub(building_type.price());
        }
        amount = amount.saturating_sub(pending);
        for _ in 0..amount {
            self.start_build(BuildParam::build(building_type))?;
        }
        Ok(())
    }

    fn pending_or_ready_base_locations(&mut self) -> Vec<TilePosition> {
        self.map
            .bases
            .iter()
            .filter(|b| {
                // Count existing bases
                self
            .units
            .mine_all
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .any(|u| u.tile_position().distance_squared(b.position) < 49)
            // Count workers on their way to build something
            || self.units.my_completed.iter().any(|u| {
                !u.get_type().is_building()
                    && u.build_type().is_resource_depot()
                    && u.target_position()
                      .expect("Build order to have a target")
                        .to_tile_position()
                        .distance_squared(b.position)
                        < 49
            })
            })
            .map(|b| b.position)
            .collect()
    }
    pub fn ensure_base_count(&mut self, amount: usize) -> Result<(), FailureReason> {
        let current_base_count = self.pending_or_ready_base_locations().len();
        for _ in 0..amount.saturating_sub(current_base_count) {
            self.start_expansion()?;
        }
        Ok(())
    }

    pub fn start_expansion(&mut self) -> Result<(), FailureReason> {
        // TODO Don't just use the first base found as main base...
        let base = self
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next()
            .ok_or(FailureReason::misc("No base left"))?
            .position();

        // TODO What if there is no base? We should just pick any spot or give up
        let bases = self.pending_or_ready_base_locations();
        let location = self
            .map
            .bases
            .iter()
            .filter(|candidate| {
                !bases
                    .iter()
                    .any(|base| base.distance_squared(candidate.position) < 49)
            })
            .min_by_key(|b| b.position.distance_squared(base.to_tile_position()))
            .expect("Expansion location to exist");
        let pos = location.position;
        self.start_build(BuildParam::build(UnitType::Zerg_Hatchery).at(pos))
    }

    pub fn start_build(&mut self, param: BuildParam) -> Result<(), FailureReason> {
        assert!(
            param.unit_type.is_building(),
            "'{:?}' is not a building",
            param.unit_type
        );
        let base = if let Some(at) = param.at {
            at
        } else {
            self.units
                .my_completed
                .iter()
                .find(|u| u.get_type().is_resource_depot() && u.completed())
                .map(|u| u.tile_position())
                .ok_or(FailureReason::misc("No base found"))?
        };
        let builders: Vec<_> = self
            .tracker
            .available_units
            .iter()
            .filter(|u| u.get_type() == param.unit_type.what_builds().0)
            .cloned()
            .collect();
        let available_gms = self.tracker.available_gms;
        let order_build = self
            .tracker
            .available_gms
            .checked_sub(param.unit_type.price())
            && self.game.can_make(None, param.unit_type).unwrap_or(false);

        if order_build
            && !self
                .game
                .can_make(None, param.unit_type)
                .map_err(|e| FailureReason::misc(format!("{e:?}")))?
        {
            return Err(FailureReason::misc("Tech not available"));
        }
        if param.unit_type.what_builds().0.is_building() {
            if order_build {
                let builder = builders
                    .iter()
                    .min_by_key(|b| b.tile_position().distance_squared(base))
                    .ok_or(FailureReason::misc("No builder found"))?;

                self.tracker.reserve_unit(builder);
                builder.morph(param.unit_type);
            }
            return Ok(());
        }

        // https://liquipedia.net/starcraft/Mining ~180 frames per trip
        let dist_fn = |u: &SUnit, target: TilePosition| if u.carrying() { 180 } else { 0 } + self.map.get_path(u.target_position().unwrap_or(u.position()), target.center()).1;
        let (builder, build_tile_pos) = if param.unit_type.is_refinery() {
            self.units
                .all()
                .iter()
                .filter(|u| u.get_type() == UnitType::Resource_Vespene_Geyser)
                .map(|u| u.tile_position())
                .min_by_key(|p| p.distance_squared(base))
                .map(|p| {
                    builders
                        .iter()
                        .min_by_key(|b| dist_fn(b, p))
                        .ok_or(FailureReason::misc("No builder found"))
                        .map(|b| (b, p))
                })
                .ok_or(FailureReason::misc("No vespene geyser found"))??
        } else {
            Rectangle::<TilePosition>::new(base - (7, 7), base + (7, 7))
                .into_iter()
                .map(|p| {
                    builders
                        .iter()
                        .min_by_key(|b| dist_fn(&b, p))
                        .map(|builder| (builder, p))
                        .ok_or(FailureReason::misc("No builder found"))
                })
                .flatten()
                .filter(|(b, p)| {
                    // !self.units.all().iter().any(|it| {
                    //     let planned_place = Rectangle::new(*p, *p + param.unit_type.tile_size());
                    //     match it.get_type() {
                    //         UnitType::Zerg_Extractor | UnitType::Resource_Vespene_Geyser => {
                    //             Rectangle::new(
                    //                 it.tile_position(),
                    //                 it.tile_position() + it.get_type().tile_size(),
                    //             )
                    //             .extrude(1)
                    //             .intersects(planned_place)
                    //         }
                    //         _ => false,
                    //     }
                    // }) &&
                    self.game
                        .can_build_here(&b.unit, *p, param.unit_type, false)
                        .unwrap_or(false)
                })
                .min_by_key(|(_, p)| p.distance_squared(base))
                .ok_or(FailureReason::misc("No build location found"))?
        };
        let dim = Position::new(
            param.unit_type.dimension_left() + param.unit_type.dimension_right() + 1,
            param.unit_type.dimension_up() + param.unit_type.dimension_down() + 1,
        );

        let build_pos =
            build_tile_pos.to_position() + param.unit_type.tile_size().to_position() / 2;
        let frames_to_start_build = self.estimate_frames_to(&builder, build_pos);
        let future_gms = available_gms + self.estimate_gms(frames_to_start_build, 1);
        // CVIS.lock().unwrap().log_unit_frame(
        //     &builder,
        //     format!(
        //         "BUILD: frames to start: {}, available gms {:?}, future gms: {:?}, ordering: {}",
        //         frames_to_start_build, available_gms, future_gms, order_build
        //     ),
        // );
        if !order_build && !(future_gms >= param.unit_type.price()) {
            CVIS.lock().unwrap().draw_rect(
                build_pos.x - param.unit_type.dimension_left(),
                build_pos.y - param.unit_type.dimension_up(),
                build_pos.x + param.unit_type.dimension_right(),
                build_pos.y + param.unit_type.dimension_down(),
                Color::Red,
            );
            CVIS.lock().unwrap().draw_text(
                build_pos.x,
                build_pos.y,
                format!(
                    "Frames: {}, GMS: {},{}",
                    frames_to_start_build, future_gms.minerals, future_gms.gas
                ),
            );
            return Err(FailureReason::InsufficientResources);
        }
        self.tracker.reserve_unit(builder);
        if !order_build || builder.position().distance_squared(build_pos) > 128 * 128 {
            CVIS.lock().unwrap().draw_rect(
                build_pos.x - param.unit_type.dimension_left(),
                build_pos.y - param.unit_type.dimension_up(),
                build_pos.x + param.unit_type.dimension_right(),
                build_pos.y + param.unit_type.dimension_down(),
                Color::Grey,
            );
            CVIS.lock().unwrap().draw_text(
                build_pos.x,
                build_pos.y,
                format!("Frames: {}", frames_to_start_build),
            );
            if builder
                .target_position()
                .map(|tp| tp.distance_squared(build_pos) > 32 * 32)
                .unwrap_or(true)
            {
                builder.move_to(build_pos).ok();
            }
        } else {
            CVIS.lock().unwrap().draw_rect(
                build_pos.x - param.unit_type.dimension_left(),
                build_pos.y - param.unit_type.dimension_up(),
                build_pos.x + param.unit_type.dimension_right(),
                build_pos.y + param.unit_type.dimension_down(),
                Color::Green,
            );
            builder
                .build(param.unit_type, build_tile_pos)
                .map_err(FailureReason::Bwapi)?;
        }
        Ok(())
    }
}
