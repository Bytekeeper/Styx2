use crate::cherry_vis::*;
use crate::cluster::*;
use crate::combat_sim::*;
use crate::MyModule;
use rsbwapi::{Position, UnitType};
use std::rc::Rc;

#[derive(Default)]
pub struct Skirmishes {
    pub skirmishes: Vec<Skirmish>,
}

pub struct Skirmish {
    pub combat_evaluation: i32,
    pub cluster: Rc<Cluster>,
    pub engaged: bool,
}

impl Skirmishes {
    pub fn new(module: &MyModule, clusters: &[Rc<Cluster>]) -> Skirmishes {
        let mut skirmishes = Vec::with_capacity(clusters.len());
        for cluster in clusters {
            // Basic idea: We simulate attacking and fleeing. What we would lose on fleeing is
            // basically the "hysteresis" for attacking
            let walkability = |x, y| {
                Position::new(x, y).is_valid(&&module.game)
                    && module.game.is_walkable((x / 8, y / 8))
            };
            let mut sim_attack = Simulator {
                player_a: Player {
                    agents: cluster
                        .units
                        .iter()
                        .filter(|u| !u.player().is_enemy())
                        .map(Agent::from_unit)
                        .collect(),
                    script: Attacker::new(),
                },
                player_b: Player {
                    agents: cluster
                        .units
                        .iter()
                        .filter(|u| u.player().is_enemy())
                        .map(Agent::from_unit)
                        .collect(),
                    script: Attacker::new(),
                },
                walkability,
            };
            let mut sim_flee = Simulator {
                player_a: Player {
                    agents: sim_attack
                        .player_a
                        .agents
                        .iter()
                        .map(|a| a.clone().with_speed_factor(0.8))
                        .collect(),
                    script: Retreater,
                },
                player_b: sim_attack.player_b.clone(),
                walkability,
            };
            let mut sim_enemy_defends = Simulator {
                player_a: sim_attack.player_a.clone(),
                player_b: Player {
                    agents: sim_attack
                        .player_b
                        .agents
                        .iter()
                        .map(|a| a.clone().with_speed_factor(0.1))
                        .collect(),
                    script: Attacker::new(),
                },
                walkability,
            };
            // TODO 8 secs ok? More, less, stacked?
            let frames = sim_attack.simulate_for(8 * 24);
            sim_flee.simulate_for(8 * 24);
            sim_enemy_defends.simulate_for(8 * 24);
            cvis().log(format!(
                "f:{frames}\n{}\nvs\n{}",
                sim_attack
                    .player_a
                    .agents
                    .iter()
                    .map(|a| format!("{:?}:{} a:{}\n", a.unit_type, a.id, a.is_alive))
                    .collect::<String>(),
                sim_attack
                    .player_b
                    .agents
                    .iter()
                    .map(|a| format!("{:?}:{} a:{}\n", a.unit_type, a.id, a.is_alive))
                    .collect::<String>()
            ));

            let my_dead_after_fleeing: i32 = sim_flee
                .player_a
                .agents
                .iter()
                .filter(|u| !u.is_alive)
                .map(|u| Self::agent_value(u))
                .sum();
            let my_dead: i32 = sim_attack
                .player_a
                .agents
                .iter()
                .filter(|u| !u.is_alive)
                .map(|u| Self::agent_value(u))
                .sum();
            let enemy_dead: i32 = sim_attack
                .player_b
                .agents
                .iter()
                .filter(|u| !u.is_alive)
                .map(|u| Self::agent_value(u))
                .sum();
            let enemy_defense_my_dead: i32 = sim_enemy_defends
                .player_a
                .agents
                .iter()
                .filter(|u| !u.is_alive)
                .map(|u| Self::agent_value(u))
                .sum();
            let enemy_defense_enemy_dead: i32 = sim_enemy_defends
                .player_b
                .agents
                .iter()
                .filter(|u| !u.is_alive)
                .map(|u| Self::agent_value(u))
                .sum();
            let combat_evaluation = enemy_dead - my_dead;
            let enemy_defense_evaluation = enemy_defense_enemy_dead - enemy_defense_my_dead;

            let engaged = cluster.units.iter().any(|u| {
                u.player().is_me()
                    && cluster.units.iter().any(|e| {
                        e.player().is_enemy()
                            && (e.is_in_weapon_range(u) || u.is_in_weapon_range(e))
                    })
            });
            skirmishes.push(Skirmish {
                combat_evaluation: combat_evaluation.min(enemy_defense_evaluation)
                    + my_dead_after_fleeing,
                cluster: cluster.clone(),
                engaged,
            });
        }

        Self { skirmishes }
    }

    // Relative "value" of an agent regarding other agents
    // TODO should be modified base on game state
    fn agent_value(a: &Agent) -> i32 {
        // Cost
        let mut res = (a.unit_type.mineral_price() + 3 * a.unit_type.gas_price())
            / (1 + a.unit_type.is_two_units_in_one_egg() as i32);
        // Zerg workers are a bit more important
        if a.unit_type == UnitType::Zerg_Drone {
            res = res * 3 / 2;
        }
        assert!(res >= 0);
        res
    }
}
