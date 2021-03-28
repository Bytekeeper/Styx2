use crate::*;

#[derive(Default)]
pub struct Gathering;

impl Gathering {
    pub fn go(&mut self, frame: &Frame) {
        let game = frame.game;
        let mut minerals: Vec<_> = game
            .get_static_minerals()
            .iter()
            .filter(|m| m.is_visible())
            .cloned()
            .collect();
        frame
            .available_units
            .iter()
            .filter(|u| !u.is_gathering_minerals() && u.get_type().is_worker())
            .filter_map(|u| {
                let m = minerals
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, m)| m.get_position().distance_squared(u.get_position()));
                if let Some((i, &m)) = m {
                    minerals.swap_remove(i);
                    Some((u, m))
                } else {
                    None
                }
            })
            .for_each(|(u, m)| {
                u.gather(&m).ok();
            });
    }
}
