use crate::cherry_vis::*;
use crate::*;
use rsbwapi::*;

pub struct Squad {
    pub units: Vec<SUnit>,
    pub target: Position,
}

impl Squad {
    pub fn engage(&self, module: &MyModule) {
        let enemies: Vec<_> = module.units.enemy.iter().collect();
        let vanguard = self
            .units
            .iter()
            .min_by_key(|u| {
                module
                    .map
                    .get_path(
                        u.position().to_walk_position(),
                        self.target.to_walk_position(),
                    )
                    .1
            })
            .unwrap();
        let uc = UnitCluster {
            vanguard,
            units: &self.units.iter().collect::<Vec<_>>(),
            vanguard_dist_to_target: module
                .map
                .get_path(
                    vanguard.position().to_walk_position(),
                    self.target.to_walk_position(),
                )
                .1,
        };
        let solution = module.select_targets(uc, enemies, self.target, false);
        for (u, t) in solution {
            if let Some(target) = &t {
                CVIS.lock().unwrap().draw_unit_pos_line(
                    &u,
                    target.position().x,
                    target.position().y,
                    Color::Red,
                );
                CVIS.lock().unwrap().draw_line(
                    u.position().x,
                    u.position().y,
                    target.position().x,
                    target.position().y,
                    Color::White,
                );
                assert!(u.exists());

                if let Err(e) = u.attack(target) {
                    CVIS.lock().unwrap().draw_text(
                        u.position().x,
                        u.position().y,
                        format!("Attack failed: {:?}", e),
                    );
                    u.stop();
                }
            } else if !u.attacking() {
                CVIS.lock().unwrap().draw_line(
                    u.position().x,
                    u.position().y,
                    self.target.x,
                    self.target.y,
                    Color::Black,
                );
                u.attack_position(self.target);
            }
        }
    }

    pub fn update(&mut self, module: &MyModule) {
        let enemies: Vec<_> = module
            .units
            .enemy
            .iter()
            .filter(|it| !it.missing() && it.completed())
            .collect();
        let vanguard = self
            .units
            .iter()
            .min_by_key(|u| {
                module
                    .map
                    .get_path(
                        u.position().to_walk_position(),
                        self.target.to_walk_position(),
                    )
                    .1
            })
            .unwrap();
        let uc = UnitCluster {
            vanguard,
            units: &self.units.iter().collect::<Vec<_>>(),
            vanguard_dist_to_target: module
                .map
                .get_path(
                    vanguard.position().to_walk_position(),
                    self.target.to_walk_position(),
                )
                .1,
        };
        struct CombatUnit {
            health: i32,
            ground: bool,
            ground_damage_per_frame: i32,
            air_damage_per_frame: i32,
        }
        let sim_frames = 48;
        let dmg = |u: &SUnit| {
            let weapon_range = u
                .get_ground_weapon()
                .max_range
                .max(u.get_air_weapon().max_range) as u32;
            let distance_to_target =
                (u.position().distance(self.target) - u.top_speed() * sim_frames as f64).max(0.0);
            // If out of range incl. some leeway for buildings, don't include in sim
            if distance_to_target > weapon_range as f64 + 64.0 {
                return None;
            }
            let damage = u.get_ground_weapon().damage;
            let cooldown = u.get_ground_weapon().weapon_type.damage_cooldown();
            if u.get_type() == UnitType::Terran_Bunker {
                damage = UnitType::Terran_Marine.ground_weapon().damage_amount();
                cooldown = UnitType::Terran_Marine.ground_weapon().damage_cooldown();
            }
            let ground_damage_per_frame = damage / cooldown;
            let damage = u.get_air_weapon().damage;
            let cooldown = u.get_air_weapon().weapon_type.damage_cooldown();
            if u.get_type() == UnitType::Terran_Bunker {
                damage = UnitType::Terran_Marine.ground_weapon().damage_amount();
                cooldown = UnitType::Terran_Marine.ground_weapon().damage_cooldown();
            }
            let air_damage_per_frame = damage / cooldown;
            Some(CombatUnit {
                health: u.hit_points() + u.shields(),
                ground: !u.flying(),
                ground_damage_per_frame,
                air_damage_per_frame,
            })
        };
        let mut my_units: Vec<_> = uc.units.iter().filter_map(|it| dmg(it)).collect();
        let mut enemy_units: Vec<_> = enemies.iter().filter_map(|it| dmg(it)).collect();
        my_units.sort_by_key(|cu| -cu.health);
        enemy_units.sort_by_key(|cu| -cu.health);

        let apply_damage = |units: &mut Vec<CombatUnit>, enemies: &[CombatUnit]| {
            let mut ground_damage = units
                .iter()
                .map(|it| it.ground_damage_per_frame * sim_frames)
                .sum();
            let mut air_damage = units
                .iter()
                .map(|it| it.ground_damage_per_frame * sim_frames)
                .sum();
            for unit in units {
                let damage = if unit.ground {
                    &mut ground_damage
                } else {
                    &mut air_damage
                };
                if *damage >= unit.health {
                    *damage -= unit.health;
                } else {
                    unit.health -= *damage;
                    *damage = 0;
                }
            }
        };
        apply_damage(&mut my_units, &enemy_units);
        apply_damage(&mut enemy_units, &my_units);

        let go = my_units.iter().filter(|u| u.health == 0).count()
            <= enemy_units.iter().filter(|u| u.health == 0).count();
        let vs = format!("{go}");
        CVIS.lock().unwrap().draw_text(
            uc.vanguard.position().x,
            uc.vanguard.position().y,
            vs.clone(),
        );
        let base = module
            .units
            .my_completed
            .iter()
            .filter(|u| u.get_type().is_resource_depot())
            .next()
            .unwrap()
            .position();
        module
            .game
            .draw_text_map(uc.vanguard.position() + Position::new(0, 10), &vs);
        if !go {
            for u in uc.units {
                if enemies.iter().any(|it| it.distance_to(*u) < 300) {
                    u.move_to(base);
                } else {
                    u.move_to(uc.vanguard.position());
                }
            }
            return;
        }
        let solution = module.select_targets(uc, enemies, self.target, false);
        for (u, t) in solution {
            if let Some(target) = &t {
                CVIS.lock().unwrap().draw_unit_pos_line(
                    &u,
                    target.position().x,
                    target.position().y,
                    Color::Red,
                );
                CVIS.lock().unwrap().draw_line(
                    u.position().x,
                    u.position().y,
                    target.position().x,
                    target.position().y,
                    Color::White,
                );
                assert!(u.exists());

                if let Err(e) = u.attack(target) {
                    CVIS.lock().unwrap().draw_text(
                        u.position().x,
                        u.position().y,
                        format!("Attack failed: {:?}", e),
                    );
                    u.stop();
                }
            } else if !u.attacking() {
                CVIS.lock().unwrap().draw_line(
                    u.position().x,
                    u.position().y,
                    self.target.x,
                    self.target.y,
                    Color::Black,
                );
                u.attack_position(self.target);
            }
        }
    }
}
