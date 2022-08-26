#[derive(Debug)]
pub enum Incomplete {
    Running,
    Failed(&'static str),
}

pub type BTResult<T = ()> = Result<T, Incomplete>;

pub fn condition(c: bool, msg: &'static str) -> BTResult {
    if c {
        Ok(())
    } else {
        Err(Incomplete::Failed(msg))
    }
}

impl Incomplete {
    pub fn when(self, cond: bool) -> BTResult {
        if cond {
            Err(self)
        } else {
            Ok(())
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum NodeStatus {
    Initial,
    Running,
    Success,
    Failure,
    Aborted,
}

impl From<bool> for NodeStatus {
    fn from(value: bool) -> Self {
        if value {
            NodeStatus::Success
        } else {
            NodeStatus::Failure
        }
    }
}

impl NodeStatus {
    pub fn invert(self) -> Self {
        match self {
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Success => NodeStatus::Failure,
            _ => self,
        }
    }
}

struct Memo {
    result: Option<NodeStatus>,
}

impl Memo {
    pub fn memo<T: Fn() -> NodeStatus>(&mut self, get: T) -> NodeStatus {
        if let Some(result) = self.result {
            result
        } else {
            let result = get();
            self.result = Some(result);
            result
        }
    }
}

macro_rules! sequence {
    ($($e:expr),*) => ({
        let mut result = crate::bt::NodeStatus::Success;
        $(result = if result == crate::bt::NodeStatus::Success { $e } else { result };)*
            result
    });
}

macro_rules! selector {
    ($($e:expr),*) => ({
        let mut result = crate::bt::NodeStatus::Failure;
        $(result = if result == crate::bt::NodeStatus::Failure { $e.into() } else { result };)*
            result
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_should_return_first_non_successful_status() {
        let result = sequence!({ NodeStatus::Running }, { NodeStatus::Failure });
        assert_eq!(result, NodeStatus::Running);
    }

    #[test]
    fn selector_should_return_first_non_failing_status() {
        let result = selector!({ NodeStatus::Failure }, { NodeStatus::Success });
        assert_eq!(result, NodeStatus::Success);
    }

    #[test]
    fn true_should_be_successful() {
        let result: NodeStatus = true.into();
        assert_eq!(result, NodeStatus::Success);
    }

    #[test]
    fn false_should_be_successful() {
        let result: NodeStatus = false.into();
        assert_eq!(result, NodeStatus::Failure);
    }

    #[test]
    fn should_invert() {
        assert_eq!(NodeStatus::Failure.invert(), NodeStatus::Success);
        assert_eq!(NodeStatus::Success.invert(), NodeStatus::Failure);
        assert_eq!(NodeStatus::Running.invert(), NodeStatus::Running);
    }

    #[test]
    fn compound_test() {
        let result = sequence!(
            selector!(NodeStatus::Failure, NodeStatus::Success),
            NodeStatus::Running
        );
        assert_eq!(result, NodeStatus::Running);
    }
}
