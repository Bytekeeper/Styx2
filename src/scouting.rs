use crate::boids::*;
use crate::*;
use rstar::AABB;

#[derive(Copy, Clone)]
pub struct ScoutParams {
    pub max_workers: i32,
    pub max_overlords: i32,
    pub max_scouts: i32,
}

impl Default for ScoutParams {
    fn default() -> Self {
        Self {
            max_workers: 0,
            max_overlords: 99999,
            max_scouts: 20,
        }
    }
}

impl MyModule {
    pub fn scout_target(&self, from: TilePosition) -> Option<TilePosition> {
        self.game
            .get_start_locations()
            .iter()
            .filter(|l| !self.game.is_explored(**l))
            .min_by_key(|p| p.distance_squared(from))
            .cloned()
    }

    pub fn perform_scouting(&mut self, params: ScoutParams) -> Result<(), FailureReason> {
        // TODO A bit too simplistic
        if self.units.enemy.iter().any(|e| e.get_type().is_building()) {
            return Ok(());
        }
        let ScoutParams {
            mut max_workers,
            mut max_overlords,
            mut max_scouts,
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
            .forward_base()
            .ok_or(FailureReason::misc("Base not found"))?
            .tile_position();

        location_targets.sort_by_key(|b| b.distance_squared(my_base));
        potential_expansions.sort_by_key(|b| b.distance_squared(my_base));

        for &base_loc in location_targets.iter().chain(potential_expansions.iter()) {
            let base_position = base_loc.to_position();
            let best_scout = scouts
                .iter()
                .min_by_key(|u| {
                    self.estimate_frames_to(u, base_position)
                        // We sent someone else there before? Nudge the search to use the same unit
                        + u.target_position().map(|p| p.distance(base_position) as u32 / 10).unwrap_or(0)
                })
                .cloned();
            if let Some(best_scout) = best_scout {
                match best_scout.get_type() {
                    UnitType::Zerg_Drone => max_workers -= 1,
                    UnitType::Zerg_Overlord => max_overlords -= 1,
                    _ => (),
                }

                // let pos = best_scout.position();
                // let mut boid_forces: Vec<_> = self
                //     .units
                //     .all_rstar
                //     .locate_in_envelope_intersecting(&AABB::from_corners(
                //         [pos.x - 300, pos.y - 300],
                //         [pos.x + 300, pos.y + 300],
                //     ))
                //     .filter(|u| u.player().is_enemy() && u.has_weapon_against(&best_scout))
                //     .map(|e| {
                //         separation(
                //             &best_scout,
                //             e,
                //             128.0 + e.weapon_against(&best_scout).max_range as f32,
                //             1.0,
                //         )
                //     })
                //     .collect();
                // if boid_forces.iter().any(|it| it.weight > 0.1) {
                //     // boid_forces.push(follow_path(self, &best_scout, base_position, 0.3));
                //     boid_forces.push(climb(self, &best_scout, 32, 32, 2.0));
                //     let target = self.positioning(&best_scout, &boid_forces);
                //     best_scout.move_to(target);
                // } else {
                best_scout.move_to(base_position);
                // }
                scouts.retain(|s| {
                    s != &best_scout
                        && (!s.get_type().is_worker() || max_workers > 0)
                        && (s.get_type() != UnitType::Zerg_Overlord || max_overlords > 0)
                });
                self.tracker.reserve_unit(&best_scout);
                max_scouts -= 1;
                if max_scouts <= 0 {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(())
    }
}
