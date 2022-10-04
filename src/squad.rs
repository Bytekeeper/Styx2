use crate::boids::*;
use crate::cherry_vis::*;
use crate::combat_sim as cs;
use crate::is_attacker;
use crate::*;
use rsbwapi::*;
use rstar::AABB;

pub struct Squad {
    pub target: Position,
    pub value_bias: i32,
    pub min_army: usize,
}

impl Squad {
    pub fn execute(&mut self, module: &mut MyModule) {
        // TODO Don't just bail without base, we could still be base trading
        let base = match module.forward_base() {
            None => return,
            Some(x) => x,
        };
        let tracker = &mut module.tracker;
        let enemies: Vec<_> = module
            .units
            .enemy
            .iter()
            .filter(|it| !it.missing())
            .collect();
        // TODO: When is our base actually in danger?
        let base_in_danger = module.skirmishes.skirmishes.iter().any(|c| {
            c.cluster.units.contains(&base)
                && c.cluster.units.iter().any(|it| {
                    it.player().is_me()
                        && it.get_type().is_building()
                        && c.cluster
                            .units
                            .iter()
                            .any(|e| e.player().is_enemy() && e.is_close_to_weapon_range(it, 64))
                })
        });
        let base = base.position();
        if base_in_danger {
            self.target = base;
        }
        // cvis().draw_text(
        //     uc.vanguard.position().x,
        //     uc.vanguard.position().y + 50,
        //     vs.clone(),
        // );
        let has_minimum_required_army = module
            .units
            .my_completed
            .iter()
            .filter(|it| {
                it.get_type().can_attack() && !it.get_type().is_worker() && it.get_type().can_move()
            })
            .count()
            >= self.min_army;
        cvis().log(format!("{has_minimum_required_army}"));
        let mut fall_backers: Vec<&SUnit> = vec![];
        let mut attackers: Vec<&SUnit> = vec![];
        for s in module.skirmishes.skirmishes.iter() {
            let building_defense_bonus = s
                .cluster
                .units
                .iter()
                .filter(|u| {
                    u.player().is_me()
                        && u.get_type().is_building()
                        && enemies.iter().any(|e| e.is_close_to_weapon_range(u, 96))
                })
                .count()
                * 200;
            let combat_eval =
                s.combat_evaluation.to_i32() + self.value_bias + (building_defense_bonus as i32);
            let should_attack = has_minimum_required_army && combat_eval == 0 || combat_eval > 0;
            cvis().log(format!("{building_defense_bonus} {should_attack}"));
            let units = s.cluster.units.iter().filter(|u| {
                u.get_type().can_move()
                    && is_attacker(u)
                    && tracker
                        .available_units
                        .iter()
                        .position(|it| u == &it)
                        .map(|i| tracker.available_units.swap_remove(i))
                        .is_some()
            });

            if should_attack {
                attackers.extend(units);
            } else {
                fall_backers.extend(units);
            }
        }

        // todo!("How can there be no vanguard?");
        let vanguard = attackers
            .iter()
            .chain(fall_backers.iter())
            .filter(|u| is_attacker(u))
            .min_by_key(|u| module.map.get_path(u.position(), self.target).1)
            .unwrap();
        // TODO Overlords will end up here too, is that ok?
        for unit in fall_backers.iter() {
            if enemies
                .iter()
                .any(|e| e.distance_to(*unit) < 300 && e.has_weapon_against(unit))
            {
                if unit.position().distance_squared(base) < 300 * 300 || unit.get_type().is_worker()
                {
                    let pos = unit.position();
                    let mut boid_forces: Vec<_> = module
                        .units
                        .all_in_range(*unit, 300)
                        .map(|o| {
                            separation(
                                &unit,
                                o,
                                32.0 + if o.player().is_enemy() && o.has_weapon_against(unit) {
                                    128.0 + o.weapon_against(unit).max_range as f32
                                } else {
                                    0.0
                                },
                                1.0,
                            )
                        })
                        .collect();
                    if boid_forces.iter().any(|it| it.weight > 0.1) {
                        boid_forces.push(climb(module, &unit, 32, 32, 1.0));
                        let target = module.positioning(&unit, &boid_forces);
                        unit.move_to(target);
                    }
                } else {
                    unit.move_to(base);
                }
            } else if unit.distance_to(*vanguard) > 64 {
                unit.move_to(vanguard.position());
            }
        }
        let tracker = &mut module.tracker;
        if attackers.is_empty() || enemies.is_empty() {
            return;
        }
        let uc = UnitCluster {
            units: &attackers.clone(),
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
                // if u.distance_to(target) < 32 + u.weapon_against(target).max_range {
                module.engage(&u, target);
                // TODO Turns out, chokepoints are a styx complete problem....
                // } else {
                //     // Otherwise fan out a bit
                //     let pos = u.position();
                //     let mut boid_forces: Vec<_> = module
                //         .units
                //         .all_rstar
                //         .locate_in_envelope_intersecting(&AABB::from_corners(
                //             [pos.x - 300, pos.y - 300],
                //             [pos.x + 300, pos.y + 300],
                //         ))
                //         .filter_map(|e| {
                //             if e == target {
                //                 // Lead enemy a bit, but home in
                //                 Some(cohesion(&u, e, 24, 0.0, 1.0))
                //             } else if e != &u {
                //                 // Fan-out to reach enemies without blocking each other
                //                 Some(separation(&u, e, 64.0, 0.5))
                //             } else {
                //                 None
                //             }
                //         })
                //         .collect();
                //     let target = module.positioning(&u, &boid_forces);
                //     u.move_to(target);
                // }
            } else if !u.attacking() {
                // CVIS.lock().unwrap().draw_line(
                //     u.position().x,
                //     u.position().y,
                //     self.target.x,
                //     self.target.y,
                //     Color::Black,
                // );
                u.attack_position(self.target);
                module.tracker.available_units.push(u);
            }
        }
    }
}
