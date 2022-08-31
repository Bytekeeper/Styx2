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
        let enemies: Vec<_> = module.units.enemy.iter().filter(|it| it.alive()).collect();
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
        let dmg = |u: &SUnit| {
            let fct = if u.get_type().is_worker() { 0.5 } else { 1.0 };
            let mut damage = u.get_ground_weapon().damage.max(u.get_air_weapon().damage);
            let mut cooldown = u
                .get_ground_weapon()
                .weapon_type
                .damage_cooldown()
                .max(u.get_air_weapon().weapon_type.damage_cooldown());
            if u.get_type() == UnitType::Terran_Bunker {
                damage = UnitType::Terran_Marine.ground_weapon().damage_amount();
                cooldown = UnitType::Terran_Marine.ground_weapon().damage_cooldown();
            }
            fct * (200 + u.hit_points() + u.shields()) as f32 * damage as f32
                / (cooldown + 1) as f32
        };
        let mut enemy_strength: f32 = enemies
            .iter()
            .filter(|it| it.position().distance_squared(uc.vanguard.position()) < 500 * 500)
            .map(|it| dmg(it))
            .sum::<f32>();
        let my_strength: f32 = uc
            .units
            .iter()
            .filter(|it| it.position().distance_squared(uc.vanguard.position()) < 500 * 500)
            .map(|it| dmg(it))
            .sum::<f32>();

        if uc.vanguard.position().distance_squared(self.target) < 600 * 600 {
            enemy_strength *= 0.7;
        }
        let base = module
            .units
            .my_completed
            .iter()
            .find(|u| u.get_type().is_resource_depot() && u.completed())
            .map(|u| u.position())
            .ok_or(FailureReason::misc("No base found"));
        if base.is_err() {
            return;
        }
        let base = base.unwrap();
        CVIS.lock().unwrap().draw_text(
            uc.vanguard.position().x,
            uc.vanguard.position().y,
            format!("{:.2} vs {:.2}", my_strength, enemy_strength),
        );
        if uc.vanguard.position().distance_squared(base) > 500 * 500 {
            if enemy_strength * 1.2 > my_strength {
                let rear_guard = self
                    .units
                    .iter()
                    .max_by_key(|u| {
                        module
                            .map
                            .get_path(
                                u.position().to_walk_position(),
                                self.target.to_walk_position(),
                            )
                            .1
                    })
                    .unwrap();
                for u in uc.units {
                    if enemies.iter().any(|it| it.distance_to(*u) < 300) {
                        u.move_to(base);
                    } else {
                        u.move_to((rear_guard.position() + uc.vanguard.position()) / 2);
                    }
                }
                return;
            }
            if my_strength * 1.2 < enemy_strength {
                for u in uc.units {
                    if enemies.iter().any(|it| it.distance_to(*u) < 300) {
                        u.move_to(base);
                    } else {
                        u.move_to(uc.vanguard.position());
                    }
                }
                return;
            }
        } else {
            self.target = base;
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
