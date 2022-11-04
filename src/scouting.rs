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
        let mut location_targets = self.bases.all();

        let my_base = self
            .forward_base()
            .ok_or(FailureReason::misc("Base not found"))?
            .tile_position();

        location_targets.sort_by(|a, b| {
            a.last_explored
                .cmp(&b.last_explored)
                .then_with(|| b.starting_location.cmp(&a.starting_location))
                .then_with(|| {
                    a.position
                        .distance_squared(my_base)
                        .cmp(&b.position.distance_squared(my_base))
                })
        });

        let location_targets: Vec<_> = location_targets.iter().map(|b| b.position).collect();
        cvis().log(format!(
            "Scouts: {}, targets: {}",
            scouts.len(),
            location_targets.len()
        ));

        for base_loc in location_targets {
            let base_position = base_loc.to_position();
            let best_scout = scouts
                .iter()
                .min_by_key(|u| {
                    self.estimate_frames_to(u, base_position)
                        // Avoid using mining workers
                        + if u.gathering() { 300 } else { 0 }
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
                if DRAW_SCOUT_TARGET {
                    cvis().draw_line(
                        best_scout.position().x,
                        best_scout.position().y,
                        base_position.x,
                        base_position.y,
                        Color::Brown,
                    );
                    cvis().draw_text(
                        best_scout.position().x,
                        best_scout.position().y - 10,
                        format!("{}", self.estimate_frames_to(&best_scout, base_position)),
                    );
                }
                self.flee(&best_scout, base_position);
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
