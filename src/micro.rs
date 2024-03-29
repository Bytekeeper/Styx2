use crate::cherry_vis::*;
use crate::cluster::WithPosition;
use crate::{boids::*, MyModule, SUnit};
use rsbwapi::{Position, UnitType};
use rstar::AABB;

impl MyModule {
    pub fn flee(&self, unit: &SUnit, toward: Position) {
        let pos = unit.position();
        let mut boid_forces: Vec<_> = self
            .units
            .all_in_range(unit, 300)
            .map(|o| {
                separation(
                    &unit,
                    o,
                    64.0 + if o.completed() && o.player().is_enemy() && o.has_weapon_against(unit) {
                        32.0 * o.top_speed() as f32 + o.weapon_against(unit).max_range as f32
                    } else {
                        0.0
                    },
                    1.0,
                )
            })
            .collect();
        // We divide by the amount to average the weight
        let amount = boid_forces.len() as f32;
        for boid in boid_forces.iter_mut() {
            boid.weight /= amount;
        }
        boid_forces.push(follow_path(self, unit, toward, 0.2));
        let target = self
            .positioning(&unit, &boid_forces)
            .unwrap_or_else(|| toward);
        unit.move_to(target).ok();
    }

    pub fn engage(&self, unit: &SUnit, enemy: &SUnit) {
        cvis().log_unit_frame(unit, || format!("ENG i{}", enemy.id()));
        let my_weapon = unit.weapon_against(enemy);
        let enemy_weapon = enemy.weapon_against(unit);
        let longer_range_and_not_slower = unit.top_speed() > 0.5
            && enemy_weapon.max_range < my_weapon.max_range
            && (enemy_weapon.cooldown >= my_weapon.cooldown
                || enemy.top_speed() <= unit.top_speed() && enemy.top_speed() > 0.5);
        let enemy_has_targeted_us = enemy.get_order_target().as_ref() == Some(unit);
        // Kite if we have time and ability
        // TODO check whether we can turn away and back again in time
        let kite = unit.cooldown() > 2 + unit.frames_to_turn_180()
            && !unit.sleeping()
        // If the enemy is targeting me, kiting should give allies more time to help
            && (longer_range_and_not_slower || enemy_has_targeted_us);
        if kite {
            cvis().log_unit_frame(unit, || format!("Kiting CD: {}", unit.cooldown()));
            let pos = unit.position();
            let mut boid_forces = vec![separation(&unit, enemy, my_weapon.max_range as f32, 1.0)];
            if !unit.flying() {
                // boid_forces.push(climb(self, &unit, 32, 32, 1.0));
                boid_forces.push(follow_path(self, &unit, enemy.position(), 2.0));
            } else {
                boid_forces.extend(
                    self.units
                        .all_rstar
                        .locate_in_envelope_intersecting(&AABB::from_corners(
                            [pos.x - 300, pos.y - 300],
                            [pos.x + 300, pos.y + 300],
                        ))
                        .map(|e| separation(&unit, e, 32.0, 0.3)),
                );
            }
            let target = self
                .positioning(&unit, &boid_forces)
                .unwrap_or_else(|| unit.position());
            unit.move_to(target).ok();
            return;
        }
        let wpn = unit.weapon_against(enemy);
        let distance = unit.distance_to(enemy).saturating_sub(wpn.max_range);
        // TODO lead/path enemy
        unit.attack(enemy).ok();
    }
}
