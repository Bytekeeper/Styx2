use crate::cherry_vis::*;
use crate::cluster::*;
use crate::combat_sim::*;
use crate::is_attacker;
use crate::{MyModule, SUnit};
use rsbwapi::{Position, UnitType};
use std::rc::Rc;

#[derive(Default)]
pub struct Skirmishes {
    pub skirmishes: Vec<Skirmish>,
}

pub struct Skirmish {
    pub combat_evaluation: CombatEvaluation,
    pub cluster: Rc<Cluster>,
    pub engaged: bool,
    pub vanguard: Option<SUnit>,
}

#[derive(Debug)]
pub struct CombatEvaluation {
    pub me_fleeing: SimResult,
    pub both_fighting: SimResult,
    pub enemy_defending: SimResult,
}

impl CombatEvaluation {
    pub fn to_i32(&self) -> i32 {
        self.both_fighting.delta().min(self.enemy_defending.delta()) + self.me_fleeing.my_dead
    }
}

// These are not unit numbers! They are the sum of lost "value" per player
#[derive(Debug)]
pub struct SimResult {
    my_dead: i32,
    enemy_dead: i32,
}

impl SimResult {
    pub fn delta(&self) -> i32 {
        self.enemy_dead - self.my_dead
    }
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

            let me_fleeing = SimResult {
                my_dead: sim_flee
                    .player_a
                    .agents
                    .iter()
                    .filter(|u| !u.is_alive)
                    .map(|u| Self::agent_value(u))
                    .sum(),
                enemy_dead: 0,
            };
            let both_fighting = SimResult {
                my_dead: sim_attack
                    .player_a
                    .agents
                    .iter()
                    .filter(|u| !u.is_alive)
                    .map(|u| Self::agent_value(u))
                    .sum(),
                enemy_dead: sim_attack
                    .player_b
                    .agents
                    .iter()
                    .filter(|u| !u.is_alive)
                    .map(|u| Self::agent_value(u))
                    .sum(),
            };
            let enemy_defending = SimResult {
                my_dead: sim_enemy_defends
                    .player_a
                    .agents
                    .iter()
                    .filter(|u| !u.is_alive)
                    .map(|u| Self::agent_value(u))
                    .sum(),
                enemy_dead: sim_enemy_defends
                    .player_b
                    .agents
                    .iter()
                    .filter(|u| !u.is_alive)
                    .map(|u| Self::agent_value(u))
                    .sum(),
            };
            let combat_evaluation = both_fighting.delta();
            let enemy_defense_evaluation = enemy_defending.delta();

            let engaged = cluster.units.iter().any(|u| {
                u.player().is_me()
                    && cluster.units.iter().any(|e| {
                        e.player().is_enemy()
                            && (e.is_in_weapon_range(u) || u.is_in_weapon_range(e))
                    })
            });
            skirmishes.push(Skirmish {
                combat_evaluation: CombatEvaluation {
                    me_fleeing,
                    enemy_defending,
                    both_fighting,
                },
                cluster: cluster.clone(),
                engaged,
                vanguard: cluster
                    .units
                    .iter()
                    .filter(|u| u.player().is_me() && is_attacker(u))
                    .map(|u| {
                        cluster
                            .units
                            .iter()
                            .filter(|u| u.player().is_enemy())
                            .map(|e| e.distance_to(u))
                            .min()
                            .map(|d| (u, d))
                    })
                    .flatten()
                    .min_by_key(|(_, d)| *d)
                    .map(|(u, d)| u)
                    .cloned(),
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
        assert!(res >= 0);
        res
    }
}
