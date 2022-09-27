use crate::{boids::*, MyModule, SUnit};
use rsbwapi::UnitType;
use rstar::AABB;

impl MyModule {
    pub fn engage(&self, unit: &SUnit, enemy: &SUnit) {
        let my_weapon = unit.weapon_against(enemy);
        let enemy_weapon = enemy.weapon_against(unit);
        let longer_range_and_not_slower = unit.top_speed() > 0.5
            && enemy_weapon.max_range < my_weapon.max_range
            && (enemy_weapon.cooldown >= my_weapon.cooldown
                || enemy.top_speed() <= unit.top_speed() && enemy.top_speed() > 0.5);
        // Kite if we have time and ability
        // TODO check whether we can turn away and back again in time
        let kite = unit.cooldown() > 2 + unit.frames_to_turn_180()
            && !unit.sleeping()
            && longer_range_and_not_slower;
        if kite {
            let pos = unit.position();
            let mut boid_forces: Vec<_> = self
                .units
                .all_rstar
                .locate_in_envelope_intersecting(&AABB::from_corners(
                    [pos.x - 300, pos.y - 300],
                    [pos.x + 300, pos.y + 300],
                ))
                .map(|e| separation(&unit, e, 32.0, 0.3))
                .collect();
            boid_forces.push(separation(&unit, enemy, my_weapon.max_range as f32, 1.0));
            if !unit.flying() {
                boid_forces.push(climb(self, &unit, 32, 32, 1.0));
            }
            let target = self.positioning(&unit, &boid_forces);
            unit.move_to(target);
            return;
        }
        unit.attack(enemy);
    }
}
