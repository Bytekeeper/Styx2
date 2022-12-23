use crate::cherry_vis::CherryVisOutput;
use crate::cluster::{dbscan, Cluster, WithPosition};
use crate::splayer::*;
use crate::MyModule;
use crate::SupplyCounter;
use crate::CVIS;
use ahash::AHashMap;
use rsbwapi::*;
use rstar::{Envelope, RTree, RTreeObject, AABB};
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
    pub all_rstar: RTree<SUnit>,
    pub clusters: Vec<Rc<Cluster>>,
}

pub fn stop_frames(unit_type: UnitType) -> i32 {
    match unit_type {
        UnitType::Terran_Goliath
        | UnitType::Terran_Siege_Tank_Tank_Mode
        | UnitType::Terran_Siege_Tank_Siege_Mode
        | UnitType::Protoss_Reaver => 1,
        UnitType::Terran_Ghost | UnitType::Zerg_Hydralisk => 3,
        UnitType::Protoss_Arbiter | UnitType::Zerg_Zergling => 4,
        UnitType::Protoss_Zealot | UnitType::Protoss_Dragoon => 7,
        UnitType::Terran_Marine | UnitType::Terran_Firebat | UnitType::Protoss_Corsair => 8,
        UnitType::Protoss_Dark_Templar | UnitType::Zerg_Devourer => 9,
        UnitType::Zerg_Ultralisk => 14,
        UnitType::Protoss_Archon => 15,
        UnitType::Terran_Valkyrie => 40,
        _ => 2,
    }
}

impl Units {
    pub fn new(game: &Game, players: &Players) -> Self {
        let mut result = Units::default();
        result.update(game, players);
        result
    }

    pub fn all(&self) -> impl Iterator<Item = &SUnit> {
        self.all.values()
    }

    pub fn update(&mut self, game: &Game, players: &Players) {
        for u in self.all.values() {
            let mut inner = u.inner.borrow_mut();
            inner.is_visible = false;
            inner.exists = false;
            inner.detected = false;
            inner.missing |= game.is_visible(inner.position.to_tile_position());
        }
        self.all.retain(|_, u| {
            let inner = u.inner.borrow();
            !inner.missing || inner.type_.is_flying_building() || !inner.type_.is_building()
        });
        for u in game.get_all_units() {
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
            let mut inner = unit.inner.borrow_mut();
            if old.pending_goal.valid_until >= game.get_frame_count() {
                inner.pending_goal = old.pending_goal;
            }
            if !inner.is_moving
                && inner
                    .target_position
                    .map(|p| p.distance_squared(inner.position) > 64 * 64)
                    .unwrap_or(false)
            {
                // dbg!(inner.position, inner.target_position);
                inner.stuck_frames = old.stuck_frames + 1;
            } else {
                inner.stuck_frames = 0;
            }
        }
        for u in self.all.values().filter(|u| u.missing()) {
            crate::cvis().log_unit_frame(u, || "Missing");
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
            .filter(|it| it.player().is_enemy())
            .cloned()
            .collect();
        self.minerals = self
            .all
            .values()
            .filter(|it| it.get_type().is_mineral_field())
            .cloned()
            .collect();
        self.all_rstar = RTree::bulk_load(
            self.all
                .values()
                .filter(|it| !it.player().is_neutral() && !it.missing())
                .cloned()
                .collect(),
        );
        self.clusters = dbscan(&self.all_rstar, 392, 4)
            .into_iter()
            .map(Rc::new)
            .collect();
    }

    pub fn mark_dead(&mut self, unit: &Unit) {
        self.all.remove(&unit.get_id());
    }

    pub fn all_in_radius(
        &self,
        position: impl Into<Position>,
        radius: i32,
    ) -> impl Iterator<Item = &SUnit> + '_ {
        let pos: Position = position.into();
        self.all_rstar
            .locate_in_envelope_intersecting(&AABB::from_corners(
                [pos.x - radius, pos.y - radius],
                [pos.x + radius, pos.y + radius],
            ))
            .filter(move |it| it.envelope().distance_2(&[pos.x, pos.y]) < radius * radius)
    }

    pub fn all_in_envelope(&self, envelope: AABB<[i32; 2]>) -> impl Iterator<Item = &SUnit> + '_ {
        self.all_rstar.locate_in_envelope_intersecting(&envelope)
    }

    pub fn all_in_range(
        &self,
        position: &impl RTreeObject<Envelope = AABB<[i32; 2]>>,
        range: i32,
    ) -> impl Iterator<Item = &SUnit> + '_ {
        let aabb = position.envelope();
        let (lower, upper) = (aabb.lower(), aabb.upper());
        self.all_rstar
            .locate_in_envelope_intersecting(&AABB::from_corners(
                [lower[0] - range, lower[1] - range],
                [upper[0] + range, upper[1] + range],
            ))
            .filter(move |it| it.envelope().distance_2(&aabb.center()) < range * range)
    }

    pub fn threats(&self, unit: &SUnit) -> Vec<SUnit> {
        self.all_in_range(unit, 300)
            .filter(|e| e.is_in_weapon_range(unit))
            .cloned()
            .collect()
    }
}

#[derive(PartialEq, Clone, Debug)]
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

impl WithPosition for SUnit {
    fn position(&self) -> Position {
        self.inner.borrow().position
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

    pub fn dimensions(&self) -> Rectangle<Position> {
        let inner = self.inner.borrow();
        let tl = inner.position - (inner.type_.dimension_left(), inner.type_.dimension_up());
        let br = inner.position + (inner.type_.dimension_right(), inner.type_.dimension_down());
        Rectangle::new(tl, br)
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

    pub fn unstick(&self) {
        let mut inner = self.inner.borrow_mut();
        if inner.stuck_frames < 8 {
            return;
        }
        dbg!("Unsticking", self.unit.get_id(), self.unit.get_type());
        inner.stuck_frames = 0;
        drop(inner);
        self.stop();
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

    pub fn ensnare_timer(&self) -> i32 {
        self.inner.borrow().ensnare_timer
    }

    pub fn lockdown_timer(&self) -> i32 {
        self.inner.borrow().lockdown_timer
    }

    pub fn stasis_timer(&self) -> i32 {
        self.inner.borrow().stasis_timer
    }

    pub fn stim_timer(&self) -> i32 {
        self.inner.borrow().stim_timer
    }

    pub fn elevation_level(&self) -> i32 {
        self.inner.borrow().elevation_level
    }

    pub fn missing(&self) -> bool {
        self.inner.borrow().missing
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
        let inner = self.inner.borrow();
        inner.carrying_gas || inner.carrying_minerals
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
        self.exists() && self.detected() && !self.stasised()
    }

    pub fn can_attack(&self, other: &SUnit) -> bool {
        other.targetable() && self.has_weapon_against(other)
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

    pub fn distance_to(&self, o: impl borrow::Borrow<SUnit>) -> i32 {
        self.unit.get_distance(&o.borrow().unit)
    }

    pub fn energy(&self) -> i32 {
        self.inner.borrow().energy
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

    pub fn is_close_to_weapon_range(&self, other: &SUnit, buffer: i32) -> bool {
        // TODO Is there really a marine in the bunker?
        if self.inner.borrow().type_ == UnitType::Terran_Bunker {
            let wpn = UnitType::Terran_Marine.ground_weapon();

            let max_range = wpn.max_range();
            let distance = self.distance_to(other);
            distance <= max_range + buffer
        } else {
            let wpn = self.weapon_against(other);
            if wpn.weapon_type == WeaponType::None {
                return false;
            }
            let distance = self.distance_to(other);
            (wpn.min_range == 0 || wpn.min_range < distance) && distance <= wpn.max_range + buffer
        }
    }

    pub fn is_in_weapon_range(&self, other: &SUnit) -> bool {
        // TODO Is there really a marine in the bunker?
        let mut max_range = if self.inner.borrow().type_ == UnitType::Terran_Bunker {
            let wpn = UnitType::Terran_Marine.ground_weapon();

            wpn.max_range()
        } else {
            let wpn = self.weapon_against(other);
            if wpn.weapon_type == WeaponType::None {
                return false;
            }
            wpn.max_range
        };
        let distance = self.distance_to(other);
        distance <= max_range
    }

    pub fn upgrade(&self, ut: UpgradeType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(|_| self.unit.upgrade(ut), PendingGoal::Upgrade(ut), 3)
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

    pub fn sleeping(&self) -> bool {
        !matches!(self.inner.borrow().pending_goal.goal, PendingGoal::Nothing)
    }

    pub fn gather(&self, o: &SUnit) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(
            |inner| {
                if inner.order_target.unwrap() == Some(o) {
                    Ok(true)
                } else {
                    self.unit.gather(&o.unit)
                }
            },
            if o.get_type().is_mineral_field() {
                PendingGoal::GatherMinerals(o.clone())
            } else {
                PendingGoal::GatherGas(o.clone())
            },
            3,
        )
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

    pub fn being_gathered(&self) -> bool {
        self.inner.borrow().being_gathered
    }

    pub fn carrying_minerals(&self) -> bool {
        let inner = self.inner.borrow();
        inner.carrying_minerals
    }

    pub fn get_order(&self) -> Order {
        self.inner.borrow().order
    }

    pub fn plagued(&self) -> bool {
        self.inner.borrow().is_plagued
    }

    pub fn visible(&self) -> bool {
        self.inner.borrow().is_visible
    }

    pub fn train(&self, t: UnitType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(
            |inner| {
                if inner.build_type() == t {
                    Ok(true)
                } else {
                    self.unit.train(t)
                }
            },
            PendingGoal::Train(t),
            3,
        )
    }

    pub fn build(&self, t: UnitType, pos: TilePosition) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(
            |inner| {
                if inner.build_type() == t {
                    Ok(true)
                } else {
                    self.unit.build(t, pos)
                }
            },
            PendingGoal::Build(t),
            3,
        )
    }

    pub fn morph(&self, t: UnitType) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(
            |inner| {
                if inner.build_type() == t {
                    Ok(true)
                } else {
                    self.unit.morph(t)
                }
            },
            PendingGoal::Morph(t),
            5,
        )
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

    pub fn frames_to_turn_180(&self) -> i32 {
        128 / self.get_type().turn_radius().max(1)
    }

    pub fn has_speed_upgrade(&self) -> bool {
        let upgrade_type = match self.get_type() {
            UnitType::Zerg_Zergling => UpgradeType::Metabolic_Boost,
            UnitType::Zerg_Hydralisk => UpgradeType::Muscular_Augments,
            UnitType::Zerg_Overlord => UpgradeType::Pneumatized_Carapace,
            UnitType::Zerg_Ultralisk => UpgradeType::Anabolic_Synthesis,
            UnitType::Protoss_Shuttle => UpgradeType::Gravitic_Thrusters,
            UnitType::Protoss_Observer => UpgradeType::Gravitic_Boosters,
            UnitType::Protoss_Zealot => UpgradeType::Leg_Enhancements,
            UnitType::Terran_Vulture => UpgradeType::Ion_Thrusters,
            _ => return false,
        };
        self.player().player.get_upgrade_level(upgrade_type) > 0
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
        let stop_frames = stop_frames(inner.type_);
        inner.act(
            |inner| {
                if inner.order_target().as_ref() == Some(unit) {
                    Ok(true)
                } else {
                    self.unit.attack(&unit.unit)
                }
            },
            PendingGoal::Attack(unit.clone()),
            stop_frames.max(2),
        )
    }

    pub fn attack_position(&self, pos: Position) -> BwResult<bool> {
        let mut inner = self.inner.borrow_mut();
        inner.act(
            |_| self.unit.attack(pos),
            PendingGoal::AttackPosition(pos),
            4,
        )
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
    pub carrying_gas: bool,
    pub carrying_minerals: bool,
    gathering_minerals: bool,
    gathering_gas: bool,
    being_gathered: bool,
    pub ground_weapon: Weapon,
    pub air_weapon: Weapon,
    pub burrowed: bool,
    pub order_target: Resolvable,
    pub target: Resolvable,
    pub is_repairing: bool,
    pub is_training: bool,
    pub is_constructing: bool,
    pub is_visible: bool,
    pub is_plagued: bool,
    pub last_seen: i32,
    pub last_attack_frame: i32,
    pub last_command_frame: i32,
    pub is_powered: bool,
    pub armor: i32,
    pub player: Resolvable<PlayerId, SPlayer>,
    pub is_flying: bool,
    pub hit_points: i32,
    pub energy: i32,
    pub shields: i32,
    // Not at expected position when last checked
    pub missing: bool,
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
    pub elevation_level: i32,
    pub stim_timer: i32,
    pub stasis_timer: i32,
    pub lockdown_timer: i32,
    pub ensnare_timer: i32,
    pub stuck_frames: i32,
}

#[derive(Debug, Clone)]
pub struct Weapon {
    pub damage: i32, // Including upgrades
    pub min_range: i32,
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
            missing: false,
            type_: unit.get_type(),
            build_type: unit.get_build_type(),
            tile_position: unit.get_tile_position(),
            position: unit.get_position(),
            is_completed: unit.is_completed(),
            player_id: unit.get_player().get_id(),
            carrying_gas: unit.is_carrying_gas(),
            carrying_minerals: unit.is_carrying_minerals(),
            gathering_minerals: unit.is_gathering_minerals(),
            gathering_gas: unit.is_gathering_gas(),
            being_gathered: unit.is_being_gathered(),
            burrowed: unit.is_burrowed(),
            order_target: unit.get_order_target().into(),
            target: unit.get_target().into(),
            is_repairing: unit.is_repairing(),
            is_training: unit.is_training(),
            is_constructing: unit.is_constructing(),
            is_visible: unit.is_visible(),
            is_plagued: unit.is_plagued(),
            last_seen: game.get_frame_count(),
            last_attack_frame: if cooldown > 0 {
                game.get_frame_count()
            } else {
                -1 // Well, not really - but we won't care for enemies for the first few seconds
            },
            is_powered: unit.is_powered(),
            // TODO What if it's a Bunker?
            ground_weapon: Weapon {
                min_range: unit.get_type().ground_weapon().min_range(),
                max_range: player.weapon_max_range(unit.get_type().ground_weapon()),
                max_hits: unit.get_type().max_ground_hits(),
                cooldown: unit.get_ground_weapon_cooldown(),
                weapon_type: unit.get_type().ground_weapon(),
                damage: player.damage(unit.get_type().ground_weapon())
                    * unit.get_type().max_ground_hits(),
            },
            // TODO What if it's a Bunker?
            air_weapon: Weapon {
                min_range: unit.get_type().air_weapon().min_range(),
                max_range: player.weapon_max_range(unit.get_type().air_weapon()),
                max_hits: unit.get_type().max_air_hits(),
                cooldown: unit.get_air_weapon_cooldown(),
                weapon_type: unit.get_type().air_weapon(),
                damage: player.damage(unit.get_type().air_weapon())
                    * unit.get_type().max_air_hits(),
            },
            armor: player.armor(unit.get_type()),
            player: player.into(),
            is_flying: unit.is_flying(),
            hit_points: unit.get_hit_points(),
            energy: unit.get_energy(),
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
            elevation_level: game.get_ground_height(unit.get_tile_position()),
            stim_timer: unit.get_stim_timer(),
            stasis_timer: unit.get_stasis_timer(),
            lockdown_timer: unit.get_lockdown_timer(),
            ensnare_timer: unit.get_ensnare_timer(),
            stuck_frames: 0,
        }
    }

    fn act(
        &mut self,
        cmd: impl Fn(&Self) -> BwResult<bool>,
        goal: PendingGoal,
        sleep: i32,
    ) -> BwResult<bool> {
        match &self.pending_goal.goal {
            PendingGoal::Nothing => {
                let result = cmd(self)?;
                if result {
                    self.pending_goal = Pending {
                        goal,
                        valid_until: self.last_seen + sleep,
                    };
                }
                Ok(result)
            }
            x if x == &goal => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn build_type(&self) -> UnitType {
        if let PendingGoal::Train(t) | PendingGoal::Build(t) | PendingGoal::Morph(t) =
            self.pending_goal.goal
        {
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

impl RTreeObject for SUnit {
    type Envelope = AABB<[i32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        let dim = self.dimensions();
        AABB::from_corners([dim.tl.x, dim.tl.y], [dim.br.x, dim.br.y])
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

pub fn is_attacker(unit: &SUnit) -> bool {
    unit.get_type().can_attack() && !unit.get_type().is_worker()
}
