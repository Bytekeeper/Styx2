use crate::*;

pub struct BuildParam {
    unit_type: UnitType,
    at: At,
}

pub enum At {
    Anywhere,
    TilePosition(TilePosition),
    DefenseChoke,
}

impl BuildParam {
    pub fn build(unit_type: UnitType) -> Self {
        Self {
            unit_type,
            at: if matches!(
                unit_type,
                UnitType::Zerg_Creep_Colony
                    | UnitType::Protoss_Photon_Cannon
                    | UnitType::Terran_Bunker
            ) {
                At::DefenseChoke
            } else {
                At::Anywhere
            },
        }
    }

    pub fn at(mut self, at: TilePosition) -> Self {
        self.at = At::TilePosition(at);
        self
    }
}

impl MyModule {
    pub fn do_extractor_trick(&mut self, build: UnitType) -> Result<(), FailureReason> {
        assert!(build.supply_required() < 2);
        // No supply left, cancel refinery if existing
        if self.tracker.available_gms.supply <= 0 {
            let refinery = self
                .units
                .mine_all
                .iter()
                .filter(|e| e.get_type().is_refinery() && !e.completed())
                .next();
            let unit = self
                .units
                .mine_all
                .iter()
                .filter(|e| e.build_type() == build && !e.completed())
                .next();
            // We started the refinery?
            if let Some(refinery) = refinery {
                // Did we also get the unit to start?
                if unit.is_some() {
                    // Done
                    return refinery
                        .cancel_morph()
                        .map(|_| ())
                        .map_err(|e| FailureReason::Bwapi(e));
                }
            } else {
                // Or we need to build an refinery
                let mut price = UnitType::Zerg_Extractor.price() + build.price();
                price.supply = 0;

                if self.tracker.available_gms > price {
                    self.start_build(BuildParam::build(UnitType::Zerg_Extractor))?;
                } else {
                    self.tracker
                        .available_gms
                        .checked_sub(UnitType::Zerg_Extractor.price());
                }
            }
        }
        // Fall through to claim a unit
        if self.tracker.available_gms.supply <= 2 {
            self.start_train(TrainParam::train(build))
        } else {
            Err(FailureReason::misc("Not enough supply"))
        }
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
        assert!(
            param.unit_type.price().gas <= 0.max(self.tracker.available_gms.gas)
                || self.has_pending_or_ready(|ut| ut.is_refinery()),
            "Not enough gas to build {:?}, and no refinery planned or built!",
            param.unit_type
        );
        self.tracker.unrealized.push(UnrealizedItem::UnitType(
            self.tracker.available_gms,
            param.unit_type,
        ));
        let base = match param.at {
            At::TilePosition(at) => at,
            At::DefenseChoke => self
                .forward_base()
                .ok_or(FailureReason::misc("Base not found"))?
                .tile_position(),
            At::Anywhere => self
                .main_base()
                .ok_or(FailureReason::misc("Base not found"))?
                .tile_position(),
        };
        let builders: Vec<_> = self
            .tracker
            .available_units
            .iter()
            .filter(|u| u.get_type() == param.unit_type.what_builds().0)
            .cloned()
            .collect();
        let available_gms = self.tracker.available_gms;

        // TODO This might send a worker although the tech is still missing (not order a build
        // though)
        let order_build = self
            .tracker
            .available_gms
            .checked_sub(param.unit_type.price())
            && self.has_requirements_for(param.unit_type);

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
                    self.game
                        .can_build_here(&b.unit, *p, param.unit_type, false)
                        .unwrap_or(false)
                        && (!self
                            .units
                            .all_in_radius(p.center(), 128)
                            .any(|it| it.get_type().is_resource_container())
                            || !self
                                .units
                                .all_in_radius(p.center(), 128)
                                .any(|it| it.get_type().is_resource_depot()))
                })
                .min_by_key(|(_, p)| match param.at {
                    At::TilePosition(at) => p.distance_squared(at),
                    At::Anywhere => p.distance_squared(base),
                    At::DefenseChoke => self
                        .map
                        .choke_points
                        .iter()
                        .map(|cp| p.distance_squared(cp.top.to_tile_position()))
                        .min()
                        .unwrap(),
                })
                .ok_or(FailureReason::misc("No build location found"))?
        };
        let dim = Position::new(
            param.unit_type.dimension_left() + param.unit_type.dimension_right() + 1,
            param.unit_type.dimension_up() + param.unit_type.dimension_down() + 1,
        );

        let build_pos =
            build_tile_pos.to_position() + param.unit_type.tile_size().to_position() / 2;
        // Account for some worker wiggling
        let frames_to_start_build = self.estimate_frames_to(&builder, build_pos) + 24;
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
                    "Frames: {}, MG: {}, {}",
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
                format!(
                    "Frames: {}, MG: {}, {}",
                    frames_to_start_build, future_gms.minerals, future_gms.gas
                ),
            );
            if builder
                .target_position()
                .map(|tp| tp.distance_squared(build_pos) > 32 * 32)
                .unwrap_or(true)
            {
                // Now this is indirection: According to PurpleWave, McRave found the (0,-7) to
                // reduce wiggling (smaller values than 7 seem to work fine too, maybe this
                // requires a test?)
                builder.move_to(build_pos - (0, 7)).ok();
            }
            let mut p = builder.position();
            let mut li: Option<usize> = None;
            let path = self.map.get_path(builder.position(), build_pos);
            for cp in path.0 {
                let top = cp.top.center();
                cvis().draw_line(p.x, p.y, top.x, top.y, Color::Green);
                if let Some(index) = li {
                    let d: u32 = self.map.distances[index][cp.index];
                    cvis().draw_text((p.x + top.x) / 2, (p.y + top.y) / 2, format!("d: {}", d));
                } else {
                    cvis().draw_text(
                        (p.x + top.x) / 2,
                        (p.y + top.y) / 2,
                        format!("d: {:.2}", top.distance(p)),
                    );
                }
                li = Some(cp.index);
                p = top;
            }
            cvis().draw_line(p.x, p.y, build_pos.x, build_pos.y, Color::Green);
            cvis().draw_text(
                (p.x + build_pos.x) / 2,
                (p.y + build_pos.y) / 2,
                format!("d: {:.2} - sum: {}", build_pos.distance(p), path.1),
            );
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
