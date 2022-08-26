use ahash::AHashMap;
use rsbwapi::Player;
use rsbwapi::*;

#[derive(Debug, Default)]
pub struct Players {
    pub all: AHashMap<PlayerId, SPlayer>,
}

impl Players {
    pub fn update(&mut self, game: &Game) {
        self.all = game
            .get_players()
            .iter()
            .map(|player| {
                let player = player.clone();
                let relation = if let Some(me) = game.self_() {
                    if me == player {
                        Relation::Me
                    } else if me.is_ally(&player) {
                        Relation::Ally
                    } else if player.is_neutral() {
                        Relation::Neutral
                    } else {
                        Relation::Enemy
                    }
                } else {
                    Relation::Neutral
                };
                (player.get_id(), SPlayer { relation, player })
            })
            .collect();
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Relation {
    Me,
    Enemy,
    Ally,
    Neutral,
}

#[derive(Debug, Clone)]
pub struct SPlayer {
    pub player: Player,
    relation: Relation,
}

impl SPlayer {
    pub fn is_enemy(&self) -> bool {
        self.relation == Relation::Enemy
    }

    pub fn is_me(&self) -> bool {
        self.relation == Relation::Me
    }
}
