use crate::cherry_vis::*;
use crate::config::*;
use crate::{MyModule, SUnit};
use glam::Vec2;
use ordered_float::OrderedFloat;
use rsbwapi::sma::Altitude;
use rsbwapi::{Position, Rectangle};

#[derive(Debug)]
pub struct WeightedPosition {
    pub weight: f32,
    pub position: Vec2,
}

impl WeightedPosition {
    pub const ZERO: Self = WeightedPosition {
        weight: 0.0,
        position: Vec2::ZERO,
    };
}

impl MyModule {
    pub fn positioning(&self, unit: &SUnit, requests: &[WeightedPosition]) -> Position {
        let mut aggregated_position = Vec2::ZERO;
        let mut aggregated_weight = 0.0;
        for &WeightedPosition { weight, position } in requests {
            aggregated_weight += weight;
            aggregated_position += position * weight;
        }
        if aggregated_weight < 0.0001 {
            return unit.position();
        }
        aggregated_position /= aggregated_weight;
        // dbg!(aggregated_position, aggregated_weight);

        // If we somehow end up "not moving at all" - use the largest weighted position (TODO Probably
        // should use more than just that)
        let target_position = unit.position()
            + if aggregated_position.length_squared() < 1.0 {
                let max_weight = requests
                    .iter()
                    .max_by_key(|wp| OrderedFloat(wp.weight))
                    .unwrap()
                    .position;
                (max_weight.x.round() as i32, max_weight.y.round() as i32)
            } else {
                (
                    aggregated_position.x.round() as i32,
                    aggregated_position.y.round() as i32,
                )
            };
        let result = if unit.flying() {
            target_position
        } else {
            self.furthest_walkable_position(unit.position(), target_position)
                .map(|p| {
                    if p == target_position.to_walk_position() {
                        target_position
                    } else {
                        p.center()
                    }
                })
                .unwrap_or_else(|| unit.position())
        };
        cvis().draw_line(
            target_position.x,
            target_position.y,
            result.x,
            result.y,
            rsbwapi::Color::Purple,
        );
        cvis().draw_line(
            unit.position().x,
            unit.position().y,
            result.x,
            result.y,
            rsbwapi::Color::Green,
        );
        result
    }
}

pub fn separation(
    unit: &SUnit,
    other: &SUnit,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    let rnd = (unit as *const SUnit as usize) as f32;
    let unit = pos_to_vec2(unit.position());
    let other = pos_to_vec2(other.position());
    let dist = unit.distance(other);
    if dist >= minimum_distance || other == unit {
        WeightedPosition::ZERO
    } else if dist == 0.0 {
        // Poor mans random
        WeightedPosition {
            position: Vec2::from_angle(rnd) * minimum_distance,
            weight,
        }
    } else {
        // "Push" more if we're closer
        let scale = 1.0 - dist / minimum_distance;
        let position = (unit - other) * (minimum_distance - dist) / dist;
        if DRAW_FORCE_VECTORS {
            cvis().draw_line(
                unit.x as i32,
                unit.y as i32,
                (unit.x + position.x) as i32,
                (unit.y + position.y) as i32,
                rsbwapi::Color::Red,
            );
        }
        WeightedPosition {
            position,
            weight: weight * scale,
        }
    }
}

pub fn cohesion(
    unit: &SUnit,
    other: &SUnit,
    prediction_frames: i32,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    goal(
        unit,
        other.predict_position(prediction_frames),
        minimum_distance,
        weight,
    )
}

pub fn goal(
    unit: &SUnit,
    target: Position,
    minimum_distance: f32,
    weight: f32,
) -> WeightedPosition {
    let target = pos_to_vec2(target);
    let unit = pos_to_vec2(unit.position());
    let dist = target.distance(unit);
    if dist == 0.0 {
        return WeightedPosition::ZERO;
    }
    let scale = 1.0 - dist / minimum_distance;
    WeightedPosition {
        position: target - unit,
        weight,
    }
}

pub fn climb(
    module: &MyModule,
    unit: &SUnit,
    range: i32,
    max_altitude: i16,
    weight: f32,
) -> WeightedPosition {
    let pos = unit.position().to_walk_position();
    let current_altitude = match module.map.get_altitude(pos) {
        Altitude::Walkable(x) => x,
        _ => 0,
    };
    if current_altitude > max_altitude {
        return WeightedPosition::ZERO;
    }
    let range = (range + 7) / 8;
    let highest_nearby_position = Rectangle::new(pos - (range, range), pos + (range, range))
        .into_iter()
        .filter(|wp| wp.is_valid(&&module.game))
        .max_by_key(|wp| match module.map.get_altitude(*wp) {
            Altitude::Walkable(x) => x as u32 * 1000000 + wp.distance_squared(pos),
            _ => 0,
        })
        .unwrap();
    let scale = (max_altitude - current_altitude) as f32 / max_altitude as f32;
    if DRAW_FORCE_VECTORS {
        cvis().draw_line(
            unit.position().x,
            unit.position().y,
            highest_nearby_position.center().x,
            highest_nearby_position.center().y,
            rsbwapi::Color::Brown,
        );
    }
    WeightedPosition {
        position: pos_to_vec2(highest_nearby_position.center() - unit.position()),
        weight: weight * scale,
    }
}

pub fn follow_path(
    module: &MyModule,
    unit: &SUnit,
    target: Position,
    weight: f32,
) -> WeightedPosition {
    let pos = unit.position();
    let result = if unit.flying() {
        WeightedPosition {
            weight,
            position: pos_to_vec2(target - pos),
        }
    } else {
        let path = module.map.get_path(unit.position(), target);
        dbg!(path.0.len(), path.1);
        let path = path.0;
        let (a, b) = match (path.get(0), path.get(1)) {
            (None, None) => (target, target),
            (Some(a), None) => (a.top.center(), target),
            (Some(a), Some(b)) => (a.top.center(), b.top.center()),
            _ => unreachable!(),
        };
        WeightedPosition {
            weight,
            position: pos_to_vec2(a) * 0.95 + pos_to_vec2(b) * 0.05 - pos_to_vec2(pos),
        }
    };
    let t = Position {
        x: result.position.x as i32,
        y: result.position.y as i32,
    } + unit.position();
    result
}

fn pos_to_vec2(pos: Position) -> Vec2 {
    Vec2::new(pos.x as f32, pos.y as f32)
}
