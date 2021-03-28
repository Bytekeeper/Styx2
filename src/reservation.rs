#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Lock<T> {
    Locked(T),
    Armed(T),
    Unlocked,
}

impl<T> Default for Lock<T> {
    fn default() -> Self {
        Lock::Unlocked
    }
}

impl<T> Lock<T> {
    pub fn lock<S: FnOnce() -> Option<T>, C: Fn(&T) -> bool>(
        &mut self,
        finder: S,
        criteria: C,
    ) -> bool {
        let mut locked = false;
        *self = match std::mem::take(self) {
            Lock::Locked(item) | Lock::Armed(item) if criteria(&item) => {
                locked = true;
                Lock::Locked(item)
            }
            old @ Lock::Armed(_) => old,
            _ => finder().map(Lock::Locked).unwrap_or(Lock::Unlocked),
        };
        locked
    }

    pub fn locked(self) -> Option<T> {
        if let Lock::Locked(item) = self {
            Some(item)
        } else {
            None
        }
    }
}
