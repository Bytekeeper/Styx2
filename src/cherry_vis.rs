use crate::SUnit;
use lazy_static::lazy_static;
use rsbwapi::{Color, Game, Unit, UnitType};
use std::panic::Location;
use std::sync::Mutex;

const LOG_FILTER: &[&'static str] = &[""];

lazy_static! {
    pub static ref CVIS: Mutex<implementation::CherryVis> =
        Mutex::new(implementation::CherryVis::default());
}

pub trait CherryVisOutput {
    fn new(game: &Game) -> Self;
    fn unit_first_seen(&mut self, unit: &Unit) {}

    fn set_frame(&mut self, frame: i32) {}
    fn draw_text(&mut self, x: i32, y: i32, text: String) {}
    fn draw_text_screen(&mut self, x: i32, y: i32, text: String) {}
    fn draw_unit_pos_line(&mut self, unit: &SUnit, x: i32, y: i32, color: Color) {}
    fn draw_line(&mut self, ax: i32, ay: i32, bx: i32, by: i32, color: Color) {}
    fn draw_rect(&mut self, ax: i32, ay: i32, bx: i32, by: i32, color: Color) {}
    fn draw_circle(&mut self, x: i32, y: i32, radius: i32, color: Color) {}
    fn log_unit_frame(&mut self, unit: &SUnit, message: String) {}
    fn log(&mut self, message: String) {}
}

pub fn cvis() -> std::sync::MutexGuard<'static, implementation::CherryVis> {
    CVIS.lock().unwrap()
}

#[cfg(not(feature = "cvis"))]
pub mod implementation {
    use super::*;

    #[derive(Default)]
    pub struct CherryVis;

    impl CherryVisOutput for CherryVis {
        fn new(_game: &Game) -> Self {
            Self
        }
    }
}

#[cfg(feature = "cvis")]
pub mod implementation {
    use super::*;
    use num_traits::cast::{FromPrimitive, ToPrimitive};
    use serde::Serialize;
    use std::collections::HashMap;
    use std::panic::Location;

    #[derive(Serialize)]
    struct DrawCommand {
        code: i32,
        args: Vec<i32>,
        r#str: String,
        cherrypi_ids_args_indices: Vec<i32>,
    }

    #[derive(Serialize)]
    struct Attachment {}

    #[derive(Serialize)]
    pub struct LogEntry {
        frame: i32,
        message: String,
        file: String,
        line: i32,
    }

    #[derive(Serialize)]
    pub struct FirstSeen {
        id: i32,
        r#type: i32,
        x: i32,
        y: i32,
    }

    #[derive(Serialize, Default)]
    pub struct CherryVis {
        _version: i32,
        types_names: HashMap<i32, String>,
        board_updates: HashMap<(), ()>,
        units_updates: HashMap<(), ()>,
        tensors_summaries: HashMap<(), ()>,
        game_values: HashMap<(), ()>,
        tasks: [(); 0],
        trees: [(); 0],
        heatmaps: [(); 0],
        draw_commands: HashMap<i32, Vec<DrawCommand>>,
        units_first_seen: HashMap<String, Vec<FirstSeen>>,
        pub logs: Vec<LogEntry>,
        units_logs: HashMap<String, Vec<LogEntry>>,
        #[serde(skip)]
        pub frame: i32,
    }

    impl CherryVisOutput for CherryVis {
        fn new(game: &Game) -> Self {
            Self {
                types_names: (0..234)
                    .map(|i| (i, UnitType::from_i32(i).unwrap().name().to_owned()))
                    .collect(),
                units_first_seen: [(
                    "1".to_string(),
                    game.get_all_units()
                        .iter()
                        .map(|u| FirstSeen {
                            id: u.get_id() as i32,
                            r#type: u.get_type() as i32,
                            x: u.get_position().x,
                            y: u.get_position().y,
                        })
                        .collect::<Vec<FirstSeen>>(),
                )]
                .into(),
                ..Default::default()
            }
        }

        fn unit_first_seen(&mut self, unit: &Unit) {
            self.units_first_seen
                .entry(self.frame.to_string())
                .or_insert_with(|| vec![])
                .push(FirstSeen {
                    id: unit.get_id() as i32,
                    r#type: unit.get_type() as i32,
                    x: unit.get_position().x,
                    y: unit.get_position().y,
                });
        }

        fn set_frame(&mut self, frame: i32) {
            self.frame = frame;
        }

        fn draw_text(&mut self, x: i32, y: i32, text: String) {
            self.draw_commands
                .entry(self.frame)
                .or_insert_with(|| vec![])
                .push(DrawCommand {
                    code: 25,
                    args: vec![x, y],
                    r#str: text,
                    cherrypi_ids_args_indices: vec![],
                });
        }

        fn draw_text_screen(&mut self, x: i32, y: i32, text: String) {
            self.draw_commands
                .entry(self.frame)
                .or_insert_with(|| vec![])
                .push(DrawCommand {
                    code: 26,
                    args: vec![x, y],
                    r#str: text,
                    cherrypi_ids_args_indices: vec![],
                });
        }

        // fn draw_unit_pos_line(&mut self, unit: &SUnit, x: i32, y: i32, color: Color) {
        //     self.draw_commands
        //         .entry(self.frame)
        //         .or_insert_with(|| vec![])
        //         .push(DrawCommand {
        //             code: 22,
        //             args: vec![unit.id() as i32, x, y, color as i32],
        //             r#str: "".to_string(),
        //             cherrypi_ids_args_indices: vec![],
        //         });
        // }

        fn draw_line(&mut self, ax: i32, ay: i32, bx: i32, by: i32, color: Color) {
            self.draw_commands
                .entry(self.frame)
                .or_insert_with(|| vec![])
                .push(DrawCommand {
                    code: 20,
                    args: vec![ax, ay, bx, by, color as i32],
                    r#str: "".to_string(),
                    cherrypi_ids_args_indices: vec![],
                });
        }

        fn draw_rect(&mut self, ax: i32, ay: i32, bx: i32, by: i32, color: Color) {
            self.draw_line(ax, ay, bx, ay, color);
            self.draw_line(bx, ay, bx, by, color);
            self.draw_line(bx, by, ax, by, color);
            self.draw_line(ax, by, ax, ay, color);
        }

        fn draw_circle(&mut self, x: i32, y: i32, radius: i32, color: Color) {
            self.draw_commands
                .entry(self.frame)
                .or_insert_with(|| vec![])
                .push(DrawCommand {
                    code: 23,
                    args: vec![x, y, radius, color as i32],
                    r#str: "".to_string(),
                    cherrypi_ids_args_indices: vec![],
                });
        }

        #[track_caller]
        fn log_unit_frame(&mut self, unit: &SUnit, message: String) {
            let loc = Location::caller();
            let f_name = loc.file().rsplitn(2, "/").next().unwrap();
            if LOG_FILTER.contains(&f_name) {
                return;
            }
            self.units_logs
                .entry(unit.id().to_string())
                .or_insert_with(|| vec![])
                .push(LogEntry {
                    frame: self.frame,
                    message: format!("{}:{} : {}", loc.line(), loc.file(), message),
                    line: loc.line() as i32,
                    file: loc.file().to_owned(),
                });
        }

        #[track_caller]
        fn log(&mut self, message: String) {
            let loc = Location::caller();
            let f_name = loc.file().rsplitn(2, "/").next().unwrap();
            if LOG_FILTER.contains(&f_name) {
                return;
            }
            self.logs.push(LogEntry {
                frame: self.frame,
                message: format!("{}:{} : {}", loc.line(), loc.file(), message),
                line: loc.line() as i32,
                file: loc.file().to_owned(),
            });
        }
    }
}
