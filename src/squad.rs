use crate::cherry_vis::*;
use crate::combat_sim as cs;
use crate::*;
use rsbwapi::*;

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

        for unit in fall_backers.iter() {
            if enemies
                .iter()
                .any(|e| e.has_weapon_against(unit) && e.distance_to(*unit) < 300)
            {
                unit.move_to(base);
            }
        }
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
        let vanguard = units
            .iter()
            .min_by_key(|u| module.map.get_path(u.position(), self.target).1)
            .unwrap();
        for unit in fall_backers.iter() {
            if !enemies
                .iter()
                .any(|e| e.has_weapon_against(unit) && e.distance_to(*unit) < 300)
            {
                unit.move_to(vanguard.position());
            }
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

                if let Err(e) = u.attack(target) {
                    CVIS.lock().unwrap().draw_text(
                        u.position().x,
                        u.position().y,
                        format!("Attack failed: {:?}", e),
                    );
                    u.stop();
                }
            } else if !u.attacking() {
                // CVIS.lock().unwrap().draw_line(
                //     u.position().x,
                //     u.position().y,
                //     self.target.x,
                //     self.target.y,
                //     Color::Black,
                // );
                u.attack_position(self.target);
            }
        }
    }
}
