use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelOutcome {
    EveryoneDraws,
    YouDraw,
    PickDraw,
    SwapHands,
    SkipNext,
    Reverse,
    PassHands,
    ThrowAway,
}

impl WheelOutcome {
    pub const ALL: [WheelOutcome; 8] = [
        WheelOutcome::EveryoneDraws,
        WheelOutcome::YouDraw,
        WheelOutcome::PickDraw,
        WheelOutcome::SwapHands,
        WheelOutcome::SkipNext,
        WheelOutcome::Reverse,
        WheelOutcome::PassHands,
        WheelOutcome::ThrowAway,
    ];
}
