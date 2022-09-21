use crate::boids::*;
use crate::cherry_vis::*;
use crate::combat_sim as cs;
use crate::*;
use rsbwapi::*;
use rstar::AABB;

pub struct Squad {
    pub target: Position,
}

impl Squad {
    pub fn update(&mut self, module: &mut MyModule) {
        let base = module
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next();
        if base.is_none() {
            // TODO: Not this way...
            return;
        }
        let base = base.unwrap().position();
        let tracker = &mut module.tracker;
        let enemies: Vec<_> = module
            .units
            .enemy
            .iter()
            .filter(|it| !it.missing())
            .collect();
        // TODO: When is our base actually in danger?
        let base_in_danger = enemies.iter().any(|it| {
            // Overlords are not really a threat to our base
            it.get_type().ground_weapon() != WeaponType::None
                && it.position().distance_squared(base) < 300 * 300
        });
        if base_in_danger {
            self.target = base;
        }
        // cvis().draw_text(
        //     uc.vanguard.position().x,
        //     uc.vanguard.position().y + 50,
        //     vs.clone(),
        // );
        let fall_backers: Vec<_> = module
            .skirmishes
            .skirmishes
            .iter()
            .filter(|s| !base_in_danger && s.combat_evaluation < 0)
            .flat_map(|s| s.cluster.units.iter().filter(|u| u.get_type().can_move()))
            .filter(|it| {
                tracker
                    .available_units
                    .iter()
                    .position(|u| it == &u)
                    .map(|i| tracker.available_units.swap_remove(i))
                    .is_some()
            })
            .collect();

        let vanguard = module
            .units
            .my_completed
            .iter()
            .filter(|it| it.get_type().can_attack())
            .min_by_key(|u| module.map.get_path(u.position(), self.target).1)
            .unwrap();
        // TODO Overlords will end up here to after scouting, is that ok?
        for unit in fall_backers.iter() {
            let pos = unit.position();
            let boid_forces: Vec<_> = module
                .units
                .all_rstar
                .locate_in_envelope_intersecting(&AABB::from_corners(
                    [pos.x - 300, pos.y - 300],
                    [pos.x + 300, pos.y + 300],
                ))
                .filter_map(|o| {
                    if o.has_weapon_against(unit) && o.player().is_enemy() {
                        Some(separation(
                            &unit,
                            o,
                            128.0 + o.weapon_against(unit).max_range as f32,
                            1.0,
                        ))
                    } else if &o != unit && o.player().is_me() {
                        Some(cohesion(&unit, o, 24, 128.0, 0.1))
                    } else {
                        None
                    }
                })
                .collect();
            if boid_forces.iter().any(|it| it.weight > 0.1) {
                let target = module.positioning(&unit, &boid_forces);
                unit.move_to(target);
            } else if unit == &vanguard {
                unit.move_to(base);
            } else {
                unit.move_to(vanguard.position());
            }
        }
        let tracker = &mut module.tracker;
        let units: Vec<_> = module
            .skirmishes
            .skirmishes
            .iter()
            .filter(|s| base_in_danger || s.combat_evaluation >= 0)
            .flat_map(|s| {
                s.cluster
                    .units
                    .iter()
                    .filter(|u| u.get_type().can_attack() && !u.get_type().is_worker())
            })
            .filter(|it| {
                tracker
                    .available_units
                    .iter()
                    .position(|u| it == &u)
                    .map(|i| tracker.available_units.swap_remove(i))
                    .is_some()
            })
            .collect();
        if units.is_empty() {
            return;
        }
        let uc = UnitCluster {
            units: &units.clone(),
            vanguard,
            vanguard_dist_to_target: module.map.get_path(vanguard.position(), self.target).1,
        };
        // module
        //     .game
        //     .draw_text_map(uc.vanguard.position() + Position::new(0, 10), &vs);
        cvis().draw_line(
            uc.vanguard.position().x,
            uc.vanguard.position().y,
            self.target.x,
            self.target.y,
            Color::Blue,
        );
        let vanguard_position = uc.vanguard.position();
        let solution = module.select_targets(uc, enemies, self.target, false);
        for (u, t) in solution {
            // if u.position().distance_squared(vanguard_position) > 300 * 300 {
            //     u.move_to(vanguard_position);
            // } else
            if let Some(target) = &t {
                // CVIS.lock().unwrap().draw_line(
                //     u.position().x,
                //     u.position().y,
                //     target.position().x,
                //     target.position().y,
                //     Color::Red,
                // );
                assert!(u.exists());
                // If close enough, engage!
                if u.distance_to(target) < 256 + u.weapon_against(target).max_range {
                    u.attack(target);
                } else {
                    // Otherwise fan out a bit
                    let pos = u.position();
                    let boid_forces: Vec<_> = module
                        .units
                        .all_rstar
                        .locate_in_envelope_intersecting(&AABB::from_corners(
                            [pos.x - 300, pos.y - 300],
                            [pos.x + 300, pos.y + 300],
                        ))
                        .filter_map(|e| {
                            if e.player().is_enemy() {
                                // Lead enemy a bit, but home in
                                Some(cohesion(&u, e, 24, 0.0, 1.0))
                            } else if e != &u {
                                // Fan-out to reach enemies without blocking each other
                                Some(separation(&u, e, 64.0, 1.0))
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            } else if !u.attacking() {
                // CVIS.lock().unwrap().draw_line(
                //     u.position().x,
                //     u.position().y,
                //     self.target.x,
                //     self.target.y,
                //     Color::Black,
                // );
                let pos = u.position();
                let boid_forces: Vec<_> = module
                    .units
                    .all_rstar
                    .locate_in_envelope_intersecting(&AABB::from_corners(
                        [pos.x - 300, pos.y - 300],
                        [pos.x + 300, pos.y + 300],
                    ))
                    .filter(|e| &&u != e)
                    .map(|e| separation(&u, e, 64.0, 1.0))
                    .collect();
                // if boid_forces.iter().any(|it| it.weight > 0.1) {
                //     let target = module.positioning(&u, &boid_forces);
                //     u.move_to(target);
                //     cvis().draw_line(pos.x, pos.y, target.x, target.y, Color::Green);
                // } else {
                u.attack_position(self.target);
                // }
            }
        }
    }
}
