use crate::cherry_vis::CherryVisOutput;
use crate::splayer::*;
use crate::MyModule;
use crate::SupplyCounter;
use crate::CVIS;
use ahash::AHashMap;
use rsbwapi::*;
use std::any::type_name;
use std::borrow;
use std::cell::Cell;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

#[derive(Debug, Default)]
pub struct Units {
    all: AHashMap<UnitId, SUnit>,
    pub minerals: Vec<SUnit>,
    pub my_completed: Vec<SUnit>,
    pub mine_all: Vec<SUnit>,
    pub enemy: Vec<SUnit>,
}

impl Units {
    pub fn new(game: &Game, players: &Players) -> Self {
        let mut result = Units::default();
        result.update(game, players);
        result
    }

    pub fn all(&self) -> Vec<&SUnit> {
        self.all.values().collect()
    }

    pub fn update(&mut self, game: &Game, players: &Players) {
        for u in self.all.values() {
            u.inner.borrow_mut().is_visible = false;
            u.inner.borrow_mut().exists = false;
        }
        for u in game.get_all_units() {
            let id = u.get_id();
            let new_unit_info = UnitInfo::new(game, &u);
            let unit = self
                .all
                .entry(u.get_id())
                .or_insert_with(|| {
                    CVIS.lock().unwrap().unit_first_seen(&u);
                    SUnit::new(new_unit_info, game, u)
                })
                .clone();

            let old = unit.inner.replace(UnitInfo::new(game, &unit.unit));
            if old.pending_goal.valid_until >= game.get_frame_count() {
                unit.inner.borrow_mut().pending_goal = old.pending_goal;
            }
        }
        for u in self.all.values() {
            u.resolve(&self.all, &players.all);
        }
        self.mine_all = self
            .all
            .values()
            .filter(|it| it.exists() && it.player().is_me())
            .cloned()
            .collect();
        self.my_completed = self
            .mine_all
            .iter()
            .filter(|it| it.completed())
            .cloned()
            .collect();
        self.enemy = self
            .all
            .values()
            .filter(|it| it.exists() && it.player().is_enemy())
            .cloned()
            .collect();
        self.minerals = self
            .all
            .values()
            .filter(|it| it.get_type().is_mineral_field())
            .cloned()
            .collect();
    }
}

#[derive(Clone, Debug)]
pub enum PendingGoal {
    Nothing,
    Build(UnitType),
    Train(UnitType),
    Morph(UnitType),
    GatherGas(SUnit),
    GatherMinerals(SUnit),
    Upgrade(UpgradeType),
    MoveTo(Position),
    AttackPosition(Position),
    Attack(SUnit),
}

#[derive(Clone, Debug)]
pub struct Pending {
    pub valid_until: i32,
    pub goal: PendingGoal,
}

#[derive(Clone, Debug)]
pub struct SUnit {
    pub unit: Unit,
    inner: Rc<RefCell<UnitInfo>>,
}

impl std::hash::Hash for SUnit {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.id().hash(hasher);
    }
}

impl SUnit {
    fn new(unit_info: UnitInfo, game: &Game, unit: Unit) -> SUnit {
        Self {
            inner: Rc::new(RefCell::new(unit_info)),
            unit,
        }
    }

    fn resolve(&self, units: &AHashMap<UnitId, SUnit>, players: &AHashMap<PlayerId, SPlayer>) {
        self.inner.borrow_mut().resolve(units, players);
    }

    pub fn id(&self) -> UnitId {
        self.unit.get_id()
    }

    pub fn player(&self) -> SPlayer {
        self.inner.borrow().player.unwrap().unwrap().clone()
    }

    pub fn get_type(&self) -> UnitType {
        self.inner.borrow().type_
    }

    pub fn idle(&self) -> bool {
        let inner = self.inner.borrow();
        match inner.pending_goal.goal {
            PendingGoal::Upgrade(_)
            | PendingGoal::Build(_)
            | PendingGoal::Morph(_)
            | PendingGoal::Train(_)
            | PendingGoal::GatherMinerals(_)
            | PendingGoal::AttackPosition(_)
            | PendingGoal::Attack(_)
            | PendingGoal::GatherGas(_) => false,
            PendingGoal::MoveTo(_) | PendingGoal::Nothing => inner.idle,
        }
    }

    pub fn completed(&self) -> bool {
        self.inner.borrow().is_completed
    }

    pub fn powered(&self) -> bool {
        self.inner.borrow().is_powered
    }

    pub fn constructing(&self) -> bool {
        self.inner.borrow().is_constructing
    }

    pub fn last_attack_frame(&self) -> i32 {
        self.inner.borrow().last_attack_frame
    }

    pub fn repairing(&self) -> bool {
        self.inner.borrow().is_repairing
    }

    pub fn carrying(&self) -> bool {
        self.inner.borrow().carrying
    }

    pub fn exists(&self) -> bool {
        self.inner.borrow().exists
    }

    pub fn burrowed(&self) -> bool {
        self.inner.borrow().burrowed
    }

    pub fn flying(&self) -> bool {
        self.inner.borrow().is_flying
    }

    pub fn being_healed(&self) -> bool {
        self.inner.borrow().is_being_healed
    }

    pub fn detected(&self) -> bool {
        self.inner.borrow().detected
    }

    pub fn stasised(&self) -> bool {
        self.inner.borrow().is_stasised
    }

    pub fn under_dark_swarm(&self) -> bool {
        self.inner.borrow().is_under_dark_swarm
    }

    pub fn under_disruption_web(&self) -> bool {
        self.inner.borrow().is_under_disruption_web
    }

    pub fn defense_matrixed(&self) -> bool {
        self.inner.borrow().is_defense_matrixed
    }

    pub fn under_storm(&self) -> bool {
        self.inner.borrow().is_under_storm
    }

    pub fn moving(&self) -> bool {
        self.inner.borrow().is_moving
    }

    pub fn braking(&self) -> bool {
        self.inner.borrow().is_braking
    }

    pub fn sieged(&self) -> bool {
        self.inner.borrow().is_sieged
    }

    pub fn last_command_frame(&self) -> i32 {
        self.inner.borrow().last_command_frame
    }

    pub fn targetable(&self) -> bool {
        self.exists() && self.detected() && self.stasised()
    }

    pub fn can_attack(&self, other: &SUnit) -> bool {
        other.targetable() && self.has_weapon_against(other)
    }

    pub fn pending_unit(&self) -> UnitType {
        match self.inner.borrow().pending_goal.goal {
            PendingGoal::Build(t) | PendingGoal::Morph(t) | PendingGoal::Train(t) => t,
            PendingGoal::Nothing
            | PendingGoal::GatherMinerals(_)
            | PendingGoal::GatherGas(_)
            | PendingGoal::Upgrade(_)
            | PendingGoal::MoveTo(_)
            | PendingGoal::AttackPosition(_)
            | PendingGoal::Attack(_) => UnitType::None,
        }
    }

    pub fn build_type(&self) -> UnitType {
        self.inner.borrow().build_type()
    }

    pub fn future_type(&self) -> UnitType {
        let t = self.build_type();
        if t != UnitType::None {
            return t;
        }
        return self.get_type();
    }

    pub fn training(&self) -> bool {
        let inner = self.inner.borrow();
        matches!(inner.pending_goal.goal, PendingGoal::Train(t)) || inner.is_training
    }

    pub fn remaining_build_time(&self) -> i32 {
        self.inner.borrow().remaining_build_time
    }

    pub fn target_position(&self) -> Option<Position> {
        let inner = self.inner.borrow();
        if let PendingGoal::MoveTo(pos) = inner.pending_goal.goal {
            return Some(pos);
        }
        inner.target_position
    }

    pub fn target(&self) -> Option<SUnit> {
        self.inner.borrow().target.unwrap().cloned()
    }

    pub fn get_order_target(&self) -> Option<SUnit> {
        self.inner.borrow().order_target()
    }

    pub fn tile_position(&self) -> TilePosition {
        self.inner.borrow().tile_position
    }

    pub fn position(&self) -> Position {
        self.inner.borrow().position
    }

    pub fn distance_to(&self, o: impl borrow::Borrow<SUnit>) -> i32 {
        self.unit.get_distance(&o.borrow().unit)
    }

    pub fn hit_points(&self) -> i32 {
        self.inner.borrow().hit_points
    }

    pub fn shields(&self) -> i32 {
        self.inner.borrow().shields
    }

    pub fn get_ground_weapon(&self) -> Weapon {
        self.inner.borrow().ground_weapon.clone()
    }

    pub fn get_air_weapon(&self) -> Weapon {
        self.inner.borrow().air_weapon.clone()
    }

    pub fn weapon_against(&self, other: &SUnit) -> Weapon {
        if other.flying() {
            self.get_air_weapon()
        } else {
            self.get_ground_weapon()
        }
    }

    pub fn has_weapon_against(&self, other: &SUnit) -> bool {
        self.weapon_against(other).weapon_type != WeaponType::None
    }

    pub fn cooldown(&self) -> i32 {
        self.get_ground_weapon()
            .cooldown
            .max(self.get_air_weapon().cooldown)
    }

    pub fn is_in_weapon_range(&self, other: &SUnit) -> bool {
        if self.inner.borrow().type_ == UnitType::Terran_Bunker {
            let wpn = UnitType::Terran_Marine.ground_weapon();

            let max_range = wpn.max_range();
            let distance = self.distance_to(other);
            distance <= max_range
        } else {
            self.unit.is_in_weapon_range(&other.unit)
        }
    }

    pub fn upgrade(&self, ut: UpgradeType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        let result = self.unit.upgrade(ut)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 3,
                goal: PendingGoal::Upgrade(ut),
            };
        }
        Ok(result)
    }

    pub fn cancel_morph(&self) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.pending_goal = Pending {
            goal: PendingGoal::Nothing,
            valid_until: inner.last_seen + 3,
        };
        self.unit.cancel_morph()
    }

    pub fn cancel_construction(&self) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.pending_goal = Pending {
            goal: PendingGoal::Nothing,
            valid_until: inner.last_seen + 3,
        };
        self.unit.cancel_construction()
    }

    pub fn stop(&self) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.pending_goal = Pending {
            goal: PendingGoal::Nothing,
            valid_until: inner.last_seen + 3,
        };
        self.unit.stop()
    }

    pub fn gather(&self, o: impl borrow::Borrow<SUnit>) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        let other = o.borrow();
        let result = self.unit.gather(&other.unit)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 3,
                goal: if other.get_type().is_mineral_field() {
                    PendingGoal::GatherMinerals(other.clone())
                } else {
                    PendingGoal::GatherGas(other.clone())
                },
            };
        }
        Ok(result)
    }

    pub fn gathering(&self) -> bool {
        self.gathering_gas() || self.gathering_minerals()
    }

    pub fn gathering_minerals(&self) -> bool {
        let inner = self.inner.borrow();
        matches!(inner.pending_goal.goal, PendingGoal::GatherMinerals(..))
            || self.inner.borrow().gathering_minerals
    }

    pub fn gathering_gas(&self) -> bool {
        let inner = self.inner.borrow();
        matches!(inner.pending_goal.goal, PendingGoal::GatherGas(..))
            || self.inner.borrow().gathering_gas
    }

    pub fn get_order(&self) -> Order {
        self.inner.borrow().order
    }

    pub fn visible(&self) -> bool {
        self.inner.borrow().is_visible
    }

    pub fn train(&self, t: UnitType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        let result = inner.build_type() == t || self.unit.train(t)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 3,
                goal: PendingGoal::Train(t),
            };
        }
        Ok(result)
    }

    pub fn build(&self, t: UnitType, pos: TilePosition) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.build_type() == t {
            return Ok(true);
        }
        if inner.sleeping() {
            return Ok(false);
        }
        let result = self.unit.build(t, pos)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 3,
                goal: PendingGoal::Build(t),
            };
        }
        Ok(result)
    }

    pub fn morph(&self, t: UnitType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        if inner.build_type() == t {
            return Ok(true);
        }
        let result = self.unit.morph(t)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 5,
                goal: PendingGoal::Morph(t),
            };
        }
        Ok(result)
    }

    pub fn sleeping(&self) -> bool {
        self.inner.borrow().sleeping()
    }

    pub fn damage_to(&self, target: &SUnit) -> i32 {
        let weapon = self.weapon_against(target);
        if weapon.weapon_type == WeaponType::None {
            return 0;
        }

        let mut damage =
            weapon.damage - target.armor() * weapon.max_hits * weapon.weapon_type.damage_factor();

        match (weapon.weapon_type.damage_type(), target.get_type().size()) {
            (DamageType::Concussive, UnitSizeType::Large) => damage /= 4,
            (DamageType::Concussive, UnitSizeType::Medium)
            | (DamageType::Explosive, UnitSizeType::Small) => damage /= 2,
            (DamageType::Explosive, UnitSizeType::Medium) => damage = 3 * damage / 4,
            _ => (),
        }
        128.min(damage)
    }

    pub fn armor(&self) -> i32 {
        self.inner.borrow().armor
    }

    pub fn top_speed(&self) -> f64 {
        // TODO modify by upgrades
        self.get_type().top_speed()
    }

    pub fn predict_position(&self, frames: i32) -> Position {
        if !self.exists() || !self.visible() || self.top_speed() < 0.001 {
            return self.position();
        }
        let frames = frames as f64;
        let v = self.unit.get_velocity();
        self.position() + Position::new((v.x * frames) as i32, (v.y * frames) as i32)
    }

    pub fn move_to(&self, pos: Position) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.target_position == Some(pos) {
            return Ok(true);
        }
        let result = self.unit.move_(pos)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 3,
                goal: PendingGoal::MoveTo(pos),
            };
        }
        Ok(result)
    }

    pub fn attacking(&self) -> bool {
        self.unit.is_attacking()
    }

    pub fn attack(&self, unit: &SUnit) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        let result = inner.order_target().as_ref() == Some(unit) || self.unit.attack(&unit.unit)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 4,
                goal: PendingGoal::Attack(unit.clone()),
            };
        }
        Ok(true)
    }

    pub fn attack_position(&self, pos: Position) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        if inner.sleeping() {
            return Ok(false);
        }
        let result = inner.target_position == Some(pos) || self.unit.attack(pos)?;
        if result {
            inner.pending_goal = Pending {
                valid_until: inner.last_seen + 4,
                goal: PendingGoal::AttackPosition(pos),
            };
        }
        Ok(true)
    }
}

impl PartialEq for SUnit {
    fn eq(&self, other: &SUnit) -> bool {
        self.unit.get_id() == other.unit.get_id()
    }
}

impl Eq for SUnit {}

#[derive(Debug)]
pub enum Resolvable<ID = UnitId, T = SUnit> {
    Unknown,
    Id(ID),
    Resolved(T),
}

impl<T> From<Player> for Resolvable<PlayerId, T> {
    fn from(p: Player) -> Self {
        Self::Id(p.id)
    }
}

impl<ID, T, I: Into<ID>> From<Option<I>> for Resolvable<ID, T> {
    fn from(s: Option<I>) -> Self {
        if let Some(id) = s {
            Self::Id(id.into())
        } else {
            Self::Unknown
        }
    }
}

impl<ID: Hash + Eq, T: Clone> Resolvable<ID, T> {
    fn unwrap(&self) -> Option<&T> {
        match self {
            Resolvable::Resolved(res) => Some(res),
            Resolvable::Unknown => None,
            _ => unreachable!("Should be resolved to {}", type_name::<T>()),
        }
    }

    fn resolve(&self, all: &AHashMap<ID, T>) -> Self {
        match self {
            Resolvable::Id(id) => {
                let res = all.get(id);
                if let Some(res) = res {
                    Self::Resolved(res.clone())
                } else {
                    Self::Unknown
                }
            }
            Resolvable::Unknown => Resolvable::Unknown,
            // Already resolved
            Resolvable::Resolved(u) => Resolvable::Resolved(u.clone()),
        }
    }
}

#[derive(Debug)]
pub struct UnitInfo {
    pub id: UnitId,
    pub type_: UnitType,
    build_type: UnitType,
    pub tile_position: TilePosition,
    pub position: Position,
    pub is_completed: bool,
    pub player_id: usize,
    pub carrying: bool,
    gathering_minerals: bool,
    gathering_gas: bool,
    pub ground_weapon: Weapon,
    pub air_weapon: Weapon,
    pub burrowed: bool,
    pub order_target: Resolvable,
    pub target: Resolvable,
    pub is_repairing: bool,
    pub is_training: bool,
    pub is_constructing: bool,
    pub is_visible: bool,
    pub last_seen: i32,
    pub last_attack_frame: i32,
    pub last_command_frame: i32,
    pub is_powered: bool,
    pub armor: i32,
    pub player: Resolvable<PlayerId, SPlayer>,
    pub is_flying: bool,
    pub hit_points: i32,
    pub shields: i32,
    pub exists: bool,
    pub is_being_healed: bool,
    pub detected: bool,
    pub is_stasised: bool,
    pub is_under_dark_swarm: bool,
    pub is_under_disruption_web: bool,
    pub is_under_storm: bool,
    pub velocity: Vector2D,
    pub is_defense_matrixed: bool,
    pub is_moving: bool,
    pub is_sieged: bool,
    pub order: Order,
    pub is_braking: bool,
    pub order_target_position: Option<Position>,
    pending_goal: Pending,
    idle: bool,
    pub interruptible: bool,
    pub target_position: Option<Position>,
    pub remaining_build_time: i32,
}

#[derive(Debug, Clone)]
pub struct Weapon {
    pub damage: i32,    // Including upgrades
    pub max_range: i32, // Including upgrades
    pub max_hits: i32,
    pub cooldown: i32, // Including upgrades
    pub weapon_type: WeaponType,
}

impl UnitInfo {
    pub fn new(game: &Game, unit: &Unit) -> Self {
        let cooldown = unit
            .get_ground_weapon_cooldown()
            .max(unit.get_air_weapon_cooldown());
        let player = unit.get_player();
        Self {
            id: unit.get_id(),
            type_: unit.get_type(),
            build_type: unit.get_build_type(),
            tile_position: unit.get_tile_position(),
            position: unit.get_position(),
            is_completed: unit.is_completed(),
            player_id: unit.get_player().get_id(),
            carrying: unit.is_carrying_gas() || unit.is_carrying_minerals(),
            gathering_minerals: unit.is_gathering_minerals(),
            gathering_gas: unit.is_gathering_gas(),
            burrowed: unit.is_burrowed(),
            order_target: unit.get_order_target().into(),
            target: unit.get_target().into(),
            is_repairing: unit.is_repairing(),
            is_training: unit.is_training(),
            is_constructing: unit.is_constructing(),
            is_visible: unit.is_visible(),
            last_seen: game.get_frame_count(),
            last_attack_frame: if cooldown > 0 {
                game.get_frame_count()
            } else {
                -1 // Well, not really - but we won't care for enemies for the first few seconds
            },
            is_powered: unit.is_powered(),
            ground_weapon: Weapon {
                max_range: player.weapon_max_range(unit.get_type().ground_weapon()),
                max_hits: unit.get_type().max_ground_hits(),
                cooldown: unit.get_ground_weapon_cooldown(),
                weapon_type: unit.get_type().ground_weapon(),
                damage: player.damage(unit.get_type().ground_weapon()),
            },
            air_weapon: Weapon {
                max_range: player.weapon_max_range(unit.get_type().air_weapon()),
                max_hits: unit.get_type().max_air_hits(),
                cooldown: unit.get_air_weapon_cooldown(),
                weapon_type: unit.get_type().air_weapon(),
                damage: player.damage(unit.get_type().air_weapon()),
            },
            armor: player.armor(unit.get_type()),
            player: player.into(),
            is_flying: unit.is_flying(),
            hit_points: unit.get_hit_points(),
            shields: unit.get_shields(),
            exists: unit.exists(),
            is_being_healed: unit.is_being_healed(),
            detected: unit.is_detected(),
            is_stasised: unit.is_stasised(),
            is_under_dark_swarm: unit.is_under_dark_swarm(),
            is_under_disruption_web: unit.is_under_disruption_web(),
            is_under_storm: unit.is_under_storm(),
            velocity: unit.get_velocity(),
            is_defense_matrixed: unit.is_defense_matrixed(),
            is_moving: unit.is_moving(),
            is_sieged: unit.is_sieged(),
            order: unit.get_order(),
            is_braking: unit.is_braking(),
            last_command_frame: unit.last_command_frame(),
            order_target_position: unit.get_order_target_position(),
            pending_goal: Pending {
                valid_until: -3,
                goal: PendingGoal::Nothing,
            },
            idle: unit.is_idle(),
            interruptible: unit.is_interruptible(),
            target_position: unit.get_target_position(),
            remaining_build_time: unit.get_remaining_build_time(),
        }
    }

    fn sleeping(&self) -> bool {
        self.pending_goal.valid_until >= self.last_seen
    }

    pub fn build_type(&self) -> UnitType {
        if let PendingGoal::Build(t) | PendingGoal::Morph(t) = self.pending_goal.goal {
            return t;
        }
        self.build_type
    }

    fn order_target(&self) -> Option<SUnit> {
        match &self.pending_goal.goal {
            PendingGoal::Morph(_)
            | PendingGoal::AttackPosition(_)
            | PendingGoal::Upgrade(_)
            | PendingGoal::MoveTo(_)
            | PendingGoal::Nothing
            | PendingGoal::Build(_)
            | PendingGoal::Train(_) => self.order_target.unwrap().cloned(),
            PendingGoal::GatherGas(u) | PendingGoal::GatherMinerals(u) | PendingGoal::Attack(u) => {
                Some(u.clone())
            }
        }
    }

    fn resolve(&mut self, units: &AHashMap<UnitId, SUnit>, players: &AHashMap<PlayerId, SPlayer>) {
        self.target = self.target.resolve(units);
        self.order_target = self.order_target.resolve(units);
        self.player = self.player.resolve(players);
    }
}

impl SupplyCounter for Vec<&UnitInfo> {
    fn get_provided_supply(&self) -> i32 {
        self.iter().fold(0, |acc, u| {
            acc + u.type_.supply_provided() + u.type_.supply_provided()
        })
    }
}

impl From<&UnitInfo> for Position {
    fn from(fu: &UnitInfo) -> Self {
        fu.position
    }
}

impl From<&SUnit> for UnitId {
    fn from(fu: &SUnit) -> Self {
        fu.id()
    }
}

pub trait IsRanged {
    fn is_ranged(&self) -> bool;
}

impl IsRanged for UnitType {
    fn is_ranged(&self) -> bool {
        self.ground_weapon().max_range() > 32
            || self.is_flyer()
            || self == &UnitType::Protoss_Reaver
    }
}
