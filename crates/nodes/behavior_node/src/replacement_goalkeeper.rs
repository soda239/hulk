// coordinator for decentralized replacement goalkeeper election

use hsl_network_messages::PlayerNumber;
use ros_z::time::Time;

pub struct ReplacementGoalkeeperInput {
    pub regular_goalkeeper_is_penalized: bool,
    pub is_playing: bool,
    pub is_eligible_candidate: bool, // temporary input until time-API understood
    pub own_player_number: PlayerNumber,
    pub lowest_claiming_player: Option<PlayerNumber>, // ToDo INCLUDING own_player_number ???
    pub election_window_elapsed: bool,
    pub now: Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplacementGoalkeeperState {
    Inactive,
    Candidate,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplacementGoalkeeperCoordinator {
    state: ReplacementGoalkeeperState,
    election_started_at: Option<Time>,
}

impl ReplacementGoalkeeperCoordinator {
    fn reset(&mut self) {
        self.state = ReplacementGoalkeeperState::Inactive;
        self.election_started_at = None;
    }

    pub fn update(&mut self, input: ReplacementGoalkeeperInput) {
        if !input.regular_goalkeeper_is_penalized {
            self.reset();
            return;
        }

        if !input.is_playing {
            self.reset();
            return;
        }

        match self.state {
            ReplacementGoalkeeperState::Inactive => {
                if input.is_eligible_candidate {
                    self.state = ReplacementGoalkeeperState::Candidate;
                    self.election_started_at = Some(input.now);
                }
            }
            ReplacementGoalkeeperState::Candidate => {
                if !input.is_eligible_candidate {
                    self.reset();
                } else {
            // Candidate -> Active
                    match (input.election_window_elapsed, input.lowest_claiming_player, input.own_player_number) {
                        (false, _, _) => {}
                        (true, None, _) => {self.state = ReplacementGoalkeeperState::Active;}
                        (true, Some(player_number), own_player_number) => {
                            if own_player_number as i8 <= player_number as i8 {
                                self.state = ReplacementGoalkeeperState::Active;
                            }
                        }
                    }  
                }
            }
            ReplacementGoalkeeperState::Active => {}
        }

        // Is there already an active claim?

        // Fallback?
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, ReplacementGoalkeeperState::Active)
        // kompakter als:
        // match self.state {
        // ReplacementGoalkeeperState::Active => true,
        // _ => false,
        // }
    }

    pub fn claims_role(&self) -> bool {
        matches!(
            self.state,
            ReplacementGoalkeeperState::Candidate | ReplacementGoalkeeperState::Active
        )
    }
}

impl Default for ReplacementGoalkeeperCoordinator {
    fn default() -> Self {
        Self {
            state: ReplacementGoalkeeperState::Inactive,
            election_started_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resets_when_regular_goalkeeper_is_not_penalized() {
        let mut coordinator = ReplacementGoalkeeperCoordinator {
            state: ReplacementGoalkeeperState::Active,
            election_started_at: None,
        };

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: false,
            is_playing: true,
            is_eligible_candidate: true,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
    }

    #[test]
    fn resets_when_robot_is_not_playing() {
        let mut coordinator = ReplacementGoalkeeperCoordinator {
            state: ReplacementGoalkeeperState::Active,
            election_started_at: None,
        };

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: false,
            is_eligible_candidate: true,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
    }

    #[test]
    fn keeps_state_when_goalkeeper_is_penalized_and_robot_is_playing() {
        let mut coordinator = ReplacementGoalkeeperCoordinator {
            state: ReplacementGoalkeeperState::Active,
            election_started_at: None,
        };

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: true,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: false,
            now: Time::zero(),
        });

        assert!(coordinator.is_active());
        assert!(coordinator.claims_role());
    }

    #[test]
    fn becomes_candidate_when_eligible() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: true,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(!coordinator.is_active());
        assert!(coordinator.claims_role());
    }

    #[test]
    fn stays_inactive_when_not_eligible() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: false,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
    }

    #[test]
    fn candidate_withdraws_when_no_longer_eligible() {
        let mut coordinator = ReplacementGoalkeeperCoordinator {
            state: ReplacementGoalkeeperState::Candidate,
            election_started_at: None,
        };

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: false,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
    }

    #[test]
    fn active_goalkeeper_stays_active_when_no_longer_eligible() {
        let mut coordinator = ReplacementGoalkeeperCoordinator {
            state: ReplacementGoalkeeperState::Active,
            election_started_at: None,
        };

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: false,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: Time::zero(),
        });

        assert!(coordinator.is_active());
        assert!(coordinator.claims_role());
    }

    #[test]
    fn stores_election_start_time_wehen_becoming_candidate() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let now = Time::zero();

        coordinator.update(ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            is_playing: true,
            is_eligible_candidate: true,
            own_player_number: PlayerNumber::Three,
            lowest_claiming_player: None,
            election_window_elapsed: true,
            now: now,
        });

        assert!(coordinator.claims_role());
        assert_eq!(coordinator.election_started_at, Some(now));
    }

    /*
    #[test]
    fn keeps_original_election_start_time_while_remaining_candidate() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();

        let start_time = <ros_z::time::Time>::Duration::from_millis(5);
        let later_time = <ros_z::time::Time>::Duration::from_millis(20);

        coordinator.update(true, true, true, start_time);
        coordinator.update(true, true, true, later_time);
    }
    */
}
