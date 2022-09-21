use crate::cherry_vis::*;
use crate::{MyModule, SUnit};
use glam::Vec2;
use ordered_float::OrderedFloat;
use rsbwapi::Position;

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
                .map(|p| p.center())
                .unwrap_or_else(|| unit.position())
        };
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
    if dist >= minimum_distance {
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
        WeightedPosition {
            position: (unit - other) * (minimum_distance - dist) / dist,
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

fn pos_to_vec2(pos: Position) -> Vec2 {
    Vec2::new(pos.x as f32, pos.y as f32)
}
