use crate::cluster::*;
use crate::combat_sim::*;
use std::rc::Rc;

#[derive(Default)]
pub struct Skirmishes {
    pub skirmishes: Vec<Skirmish>,
}

pub struct Skirmish {
    pub combat_evaluation: i32,
    pub cluster: Rc<Cluster>,
}

impl Skirmishes {
    pub fn new(clusters: &[Rc<Cluster>]) -> Skirmishes {
        let mut skirmishes = Vec::with_capacity(clusters.len());
        for cluster in clusters {
            // Basic idea: We simulate attacking and fleeing. What we would lose on fleeing is
            // basically the "hysteresis" for attacking
            let mut sim_attack = Simulator {
                player_a: Player {
                    agents: cluster
                        .units
                        .iter()
                        .filter(|u| !u.player().is_enemy())
                        .map(Agent::from_unit)
                        .collect(),
                    script: Attacker,
                },
                player_b: Player {
                    agents: cluster
                        .units
                        .iter()
                        .filter(|u| u.player().is_enemy())
                        .map(Agent::from_unit)
                        .collect(),
                    script: Attacker,
                },
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
            };
            // TODO 8 secs ok? More, less, stacked?
            sim_attack.simulate_for(8 * 24);
            sim_flee.simulate_for(8 * 24);

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
            let combat_evaluation = enemy_dead + my_dead_after_fleeing - my_dead;

            skirmishes.push(Skirmish {
                combat_evaluation,
                cluster: cluster.clone(),
            });
        }

        Self { skirmishes }
    }

    // Relative "value" of an agent regarding other agents
    // TODO should be modified base on game state
    fn agent_value(a: &Agent) -> i32 {
        let res = (a.unit_type.mineral_price() + 3 * a.unit_type.gas_price())
            / (1 + a.unit_type.is_two_units_in_one_egg() as i32);
        assert!(res > 0);
        res
    }
}
