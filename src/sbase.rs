use crate::splayer::SPlayer;
use crate::sunit::SUnit;

pub struct SBase {
    pub player: SPlayer,
    pub last_scouted: i32,
    pub resource_depot: Option<SUnit>,
}
