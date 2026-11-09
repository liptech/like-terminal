use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_PANES: usize = 8;
pub const MIN_RATIO: f64 = 0.1;
pub const MAX_RATIO: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PaneLayout {
    Leaf {
        session: Uuid,
    },
    Split {
        axis: SplitAxis,
        ratio: f64,
        start: Box<PaneLayout>,
        end: Box<PaneLayout>,
    },
}

impl PaneLayout {
    pub fn leaf(session: Uuid) -> Self {
        Self::Leaf { session }
    }

    pub fn split(axis: SplitAxis, ratio: f64, start: PaneLayout, end: PaneLayout) -> Self {
        Self::Split {
            axis,
            ratio: ratio.clamp(MIN_RATIO, MAX_RATIO),
            start: Box::new(start),
            end: Box::new(end),
        }
    }

    pub fn panes(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { start, end, .. } => start.panes() + end.panes(),
        }
    }

    pub fn sessions(&self) -> Vec<Uuid> {
        let mut sessions = Vec::new();
        self.collect_sessions(&mut sessions);
        sessions
    }

    pub fn first_session(&self) -> Option<Uuid> {
        match self {
            Self::Leaf { session } => Some(*session),
            Self::Split { start, .. } => start.first_session(),
        }
    }

    pub fn normalize(self) -> Self {
        self.trimmed(MAX_PANES)
    }

    fn collect_sessions(&self, sessions: &mut Vec<Uuid>) {
        match self {
            Self::Leaf { session } => sessions.push(*session),
            Self::Split { start, end, .. } => {
                start.collect_sessions(sessions);
                end.collect_sessions(sessions);
            }
        }
    }

    fn trimmed(self, budget: usize) -> Self {
        match self {
            Self::Leaf { .. } => self,
            Self::Split {
                axis,
                ratio,
                start,
                end,
            } => {
                let start = start.trimmed(budget);
                let used = start.panes();
                if used >= budget {
                    return start;
                }
                let end = end.trimmed(budget - used);
                Self::Split {
                    axis,
                    ratio: if ratio.is_finite() {
                        ratio.clamp(MIN_RATIO, MAX_RATIO)
                    } else {
                        0.5
                    },
                    start: Box::new(start),
                    end: Box::new(end),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn sample() -> PaneLayout {
        PaneLayout::split(
            SplitAxis::Horizontal,
            0.4,
            PaneLayout::leaf(uuid(1)),
            PaneLayout::split(
                SplitAxis::Vertical,
                0.6,
                PaneLayout::leaf(uuid(2)),
                PaneLayout::leaf(uuid(3)),
            ),
        )
    }

    #[test]
    fn round_trips_through_json() {
        let tree = sample();
        let json = serde_json::to_string(&tree).expect("serializa");
        let parsed: PaneLayout = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(parsed, tree);
        assert_eq!(parsed.panes(), 3);
        assert_eq!(parsed.sessions(), vec![uuid(1), uuid(2), uuid(3)]);
        assert_eq!(parsed.first_session(), Some(uuid(1)));
    }

    #[test]
    fn clamps_ratio_into_range() {
        let tree = PaneLayout::split(
            SplitAxis::Horizontal,
            4.0,
            PaneLayout::leaf(uuid(1)),
            PaneLayout::leaf(uuid(2)),
        );
        let PaneLayout::Split { ratio, .. } = tree else {
            panic!("esperava split");
        };
        assert!((ratio - MAX_RATIO).abs() < f64::EPSILON);
    }

    #[test]
    fn collapses_trees_bigger_than_the_limit() {
        let mut tree = PaneLayout::leaf(uuid(1));
        for index in 0..MAX_PANES + 4 {
            tree = PaneLayout::split(
                SplitAxis::Vertical,
                0.5,
                tree,
                PaneLayout::leaf(uuid(100 + index as u128)),
            );
        }
        let trimmed = tree.normalize();
        assert_eq!(trimmed.panes(), MAX_PANES);
        assert_eq!(trimmed.first_session(), Some(uuid(1)));
    }

    #[test]
    fn keeps_single_leaf_untouched() {
        let tree = PaneLayout::leaf(uuid(9));
        assert_eq!(tree.clone().normalize(), tree);
        assert_eq!(tree.panes(), 1);
        assert_eq!(tree.first_session(), Some(uuid(9)));
    }
}
