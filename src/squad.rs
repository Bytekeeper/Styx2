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
                        && c.cluster.units.iter().any(|e| {
                            !e.get_type().is_worker()
                                && e.player().is_enemy()
                                && e.is_close_to_weapon_range(it, 128)
                        })
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
        cvis().log(format!("min_army: {has_minimum_required_army}"));
        let mut fall_backers: Vec<&SUnit> = vec![];
        let mut attackers: Vec<&SUnit> = vec![];
        for s in module.skirmishes.skirmishes.iter() {
            let combat_eval =
                s.combat_evaluation.to_i32() + self.value_bias + s.potential_building_loss.my_dead;
            let should_attack = has_minimum_required_army && combat_eval == 0 || combat_eval > 0;
            cvis().log(format!(
                "building defense: {}, attack: {should_attack}",
                s.potential_building_loss.my_dead
            ));
            let tracker = &mut module.tracker;
            for unit in s.cluster.units.iter().filter(|u| {
                u.get_type().can_move()
                    && !u.get_type().is_worker()
                    && tracker.try_reserve_unit(*u).is_some()
            }) {
                if is_attacker(unit) && should_attack {
                    cvis().log_unit_frame(unit, format!("ATK {combat_eval}"));
                    attackers.push(unit);
                } else {
                    cvis().log_unit_frame(unit, format!("FB {combat_eval}"));
                    fall_backers.push(unit);
                }
            }
        }

        // todo!("How can there be no vanguard?");
        let vanguard = attackers
            .iter()
            .chain(fall_backers.iter())
            .filter(|u| is_attacker(u))
            .min_by_key(|u| module.map.get_path(u.position(), self.target).1);
        let vanguard = match vanguard {
            Some(x) => x,
            None => return, // TODO MEH
        };
        for unit in fall_backers.iter() {
            if enemies.iter().any(|e| e.frames_to_engage(unit, 32) < 48) {
                module.flee(unit, base);
            } else if unit.distance_to(*vanguard) > 64 || !unit.get_type().can_attack() {
                unit.move_to(vanguard.position());
            } else {
                let target = module
                    .units
                    .all_in_range(*unit, 300)
                    .filter(|e| {
                        if !e.player().is_enemy() || e.has_weapon_against(unit) {
                            return false;
                        }
                        let pos = if unit.is_in_weapon_range(e) {
                            unit.position()
                        } else {
                            unit.position()
                                - (unit.position() - e.position())
                                    * unit.weapon_against(e).max_range
                                    / e.position().distance(unit.position()) as i32
                        };
                        !module.units.all_in_range(*unit, 300).any(|e| {
                            e.completed()
                                && e.player().is_enemy()
                                && e.weapon_against(unit).max_range
                                    >= e.position().distance(pos) as i32
                        })
                    })
                    .min_by_key(|u| {
                        // Try to favor pylons a bit, that might be all that holds up a wall
                        u.position().distance(unit.position()) as i32
                            + if u.get_type() != UnitType::Protoss_Pylon {
                                128
                            } else {
                                0
                            }
                    });
                if let Some(target) = target {
                    cvis().draw_line(
                        unit.position().x,
                        unit.position().y,
                        target.position().x,
                        target.position().y,
                        Color::Red,
                    );
                    module.engage(unit, target);
                }
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
        assert!(!attackers
            .iter()
            .any(|a| solution.iter().find(|(u, _)| &u == a).is_none()));
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
                module.engage(&u, target);
            } else if !u.attacking() {
                // CVIS.lock().unwrap().draw_line(
                //     u.position().x,
                //     u.position().y,
                //     self.target.x,
                //     self.target.y,
                //     Color::Black,
                // );
                cvis().log_unit_frame(&u, format!("ATK POS {}", self.target));
                u.attack_position(self.target);
                module.tracker.available_units.push(u);
            }
        }
    }
}
