use crate::cherry_vis::*;
use crate::combat_sim as cs;
use crate::*;
use rsbwapi::*;

pub struct Squad {
    pub units: Vec<SUnit>,
    pub target: Position,
}

impl Squad {
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
            .min_by_key(|u| module.map.get_path(u.position(), self.target).1)
            .unwrap();
        let uc = UnitCluster {
            vanguard,
            units: &self.units.iter().collect::<Vec<_>>(),
            vanguard_dist_to_target: module.map.get_path(vanguard.position(), self.target).1,
        };
        let mut simulator = cs::Simulator {
            player_a: cs::Player {
                agents: self.units.iter().map(|u| cs::Agent::from_unit(u)).collect(),
                script: cs::Attacker,
            },
            player_b: cs::Player {
                agents: enemies
                    .iter()
                    .map(|u| {
                        let mut agent = cs::Agent::from_unit(u);
                        if u.exists() && u.gathering() {
                            agent.sleep_timer = 9999;
                        }
                        if !u.get_type().can_attack() {
                            agent.attack_target_priority = cs::TargetingPriority::Medium;
                        }
                        agent
                    })
                    .collect(),
                script: cs::Attacker,
            },
        };
        let mut flee_sim = cs::Simulator {
            player_a: cs::Player {
                agents: simulator
                    .player_a
                    .agents
                    .iter()
                    .map(|a| a.clone().with_speed_factor(0.8))
                    .collect(),
                script: cs::Retreater,
            },
            player_b: simulator.player_b.clone(),
        };
        let pos_a: Vec<_> = simulator
            .player_a
            .agents
            .iter()
            .map(|a| (a.x, a.y))
            .collect();
        let pos_b: Vec<_> = simulator
            .player_b
            .agents
            .iter()
            .map(|a| (a.x, a.y))
            .collect();
        let engaged = uc.units.iter().any(|u| {
            enemies
                .iter()
                .any(|e| u.is_in_weapon_range(e) || e.is_in_weapon_range(u))
        });
        flee_sim.simulate_for(8 * 24);
        for i in 0..8 {
            // cvis().log(format!(
            //     "{}\n###\n{}",
            //     simulator
            //         .player_a
            //         .agents
            //         .iter()
            //         .map(|u| format!(
            //             "{:?}, alive: {} h:{}, s:{}, sl: {}, cd: {}",
            //             u.unit_type,
            //             u.is_alive,
            //             u.health(),
            //             u.shields(),
            //             u.sleep_timer,
            //             u.cooldown
            //         )
            //         .split_once('_')
            //         .unwrap()
            //         .1
            //         .to_string())
            //         .collect::<String>(),
            //     simulator
            //         .player_b
            //         .agents
            //         .iter()
            //         .map(|u| format!(
            //             "{:?}, alive: {} h:{}, s: {}, sl: {}, cd: {}\n",
            //             u.unit_type,
            //             u.is_alive,
            //             u.health(),
            //             u.shields(),
            //             u.sleep_timer,
            //             u.cooldown
            //         )
            //         .split_once('_')
            //         .unwrap()
            //         .1
            //         .to_string())
            //         .collect::<String>()
            // ));
            simulator.simulate_for(24);
        }
        // TODO Very simple here, should be based on current state of the game and strategy
        // ie. an All-In can afford to lose more value, other tactics require stalling and should
        // not take heavy loses.
        let rating = |u: &cs::Agent| {
            let res = (u.unit_type.mineral_price() + 3 * u.unit_type.gas_price())
                / (1 + u.unit_type.is_two_units_in_one_egg() as i32);
            assert!(res > 0);
            res
        };
        // for agents in [
        //     (simulator.player_a.agents.iter(), pos_a),
        //     (simulator.player_b.agents.iter(), pos_b),
        // ] {
        //     for (u, old) in agents.0.zip(agents.1) {
        //         cvis().draw_line(u.x, u.y, old.0, old.1, Color::White);
        //         cvis().draw_circle(
        //             u.x,
        //             u.y,
        //             4,
        //             if u.is_alive { Color::Green } else { Color::Red },
        //         );
        //         // cvis().draw_text(
        //         //     u.x,
        //         //     u.y + 10,
        //         //     format!("{:?}: {}", u.unit_type, rating(u))
        //         //         .split_once('_')
        //         //         .unwrap()
        //         //         .1
        //         //         .to_string(),
        //         // );
        //     }
        // }
        // TODO What units of the squad have a mostly negative impact and could be pulled back?
        let cannon_fodder: Vec<_> = simulator
            .player_a
            .agents
            .iter()
            .filter_map(|u| {
                if !u.is_alive && u.attack_counter == 0 {
                    Some(u.id)
                } else {
                    None
                }
            })
            .collect();
        let my_dead_after_fleeing: i32 = flee_sim
            .player_a
            .agents
            .iter()
            .filter(|u| !u.is_alive)
            .map(|u| rating(u))
            .sum();
        let my_dead: i32 = simulator
            .player_a
            .agents
            .iter()
            .filter(|u| !u.is_alive)
            .map(|u| rating(u))
            .sum();
        let enemy_dead: i32 = simulator
            .player_b
            .agents
            .iter()
            .filter(|u| !u.is_alive)
            .map(|u| rating(u))
            .sum();
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
        // TODO: When is our base actually in danger?
        let base_in_danger = enemies.iter().any(|it| {
            // Overlords are not really a threat to our base
            it.get_type().ground_weapon() != WeaponType::None
                && it.position().distance_squared(base) < 300 * 300
        });
        if base_in_danger {
            self.target = base;
        }
        // "Hysteresis": If I don't lose more value fighting than fleeing, fight!
        let go = base_in_danger || my_dead - enemy_dead <= my_dead_after_fleeing;
        let vs = format!(
            "atk: {go}: {my_dead} - {enemy_dead} <= {my_dead_after_fleeing} {}",
            cannon_fodder.len()
        );
        cvis().draw_text(
            uc.vanguard.position().x,
            uc.vanguard.position().y + 50,
            vs.clone(),
        );
        module
            .game
            .draw_text_map(uc.vanguard.position() + Position::new(0, 10), &vs);
        cvis().draw_line(
            uc.vanguard.position().x,
            uc.vanguard.position().y,
            self.target.x,
            self.target.y,
            Color::Blue,
        );
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
                // CVIS.lock().unwrap().draw_unit_pos_line(
                //     &u,
                //     target.position().x,
                //     target.position().y,
                //     Color::Red,
                // );
                CVIS.lock().unwrap().draw_line(
                    u.position().x,
                    u.position().y,
                    target.position().x,
                    target.position().y,
                    Color::Red,
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
