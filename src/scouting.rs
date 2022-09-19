use crate::*;

#[derive(Copy, Clone)]
pub struct ScoutParams {
    pub max_workers: i32,
    pub max_overlords: i32,
}

impl Default for ScoutParams {
    fn default() -> Self {
        Self {
            max_workers: 0,
            max_overlords: 99999,
        }
    }
}

impl MyModule {
    pub fn scout(&mut self, params: ScoutParams) -> Result<(), FailureReason> {
        let ScoutParams {
            mut max_workers,
            mut max_overlords,
        } = params;
        let mut scouts: Vec<_> = self
            .tracker
            .available_units
            .iter()
            .filter(|u| {
                u.top_speed() > 0.5
                    && match u.get_type() {
                        UnitType::Zerg_Drone => params.max_workers > 0,
                        UnitType::Zerg_Overlord => params.max_overlords > 0,
                        _ => true,
                    }
            })
            .cloned()
            .collect();
        // TODO take "last scouted time/frame" into account
        let mut location_targets: Vec<_> = self
            .game
            .get_start_locations()
            .iter()
            .filter(|l| !self.game.is_explored(**l))
            .cloned()
            .collect();
        // TODO maybe consider scouting enemy expansions first?
        let mut potential_expansions: Vec<_> = self
            .map
            .bases
            .iter()
            .map(|b| b.position)
            .filter(|l| !self.game.is_explored(*l))
            .collect();
        let my_base = self
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next()
            .ok_or(FailureReason::misc("Base not found"))?
            .tile_position();

        location_targets.sort_by_key(|b| b.distance_squared(my_base));
        potential_expansions.sort_by_key(|b| b.distance_squared(my_base));

        for &base_loc in location_targets
            .iter()
            .chain(potential_expansions.iter().take(2))
        {
            let base_position = base_loc.to_position();
            let best_scout = scouts
                .iter()
                .min_by_key(|u| {
                    self.estimate_frames_to(u, base_position)
                        + u.tile_position().distance(base_loc) as u32
                })
                .cloned();
            if let Some(best_scout) = best_scout {
                match best_scout.get_type() {
                    UnitType::Zerg_Drone => max_workers -= 1,
                    UnitType::Zerg_Overlord => max_overlords -= 1,
                    _ => (),
                }
                best_scout.move_to(base_position);
                scouts.retain(|s| {
                    s != &best_scout
                        && (!s.get_type().is_worker() || max_workers > 0)
                        && (s.get_type() != UnitType::Zerg_Overlord || max_overlords > 0)
                });
                self.tracker.reserve_unit(&best_scout);
            } else {
                break;
            }
        }
        Ok(())
    }
}
