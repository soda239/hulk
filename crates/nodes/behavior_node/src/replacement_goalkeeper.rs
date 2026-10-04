// coordinator for decentralized replacement goalkeeper election

use hsl_network_messages::{Player, PlayerNumber};
use ros_z::time::Time;
use std::time::Duration;

use types::{
    players::Players,
    primary_state::PrimaryState,
    world_state::{self, WorldState},
};

pub struct ReplacementGoalkeeperInput {
    pub regular_goalkeeper_is_penalized: bool,
    pub primary_state: PrimaryState,
    pub own_player_number: PlayerNumber,

    pub available_field_players: Players<bool>, // Players on field. Info gathered via StateMessages?
    pub is_eligible_candidate: bool, // Ich bin verfügbar, meine Pose ist bekannt, ich bin nicht ballzuständig
    pub should_claim_first: bool, // unter allen verfügbaren Robotern mit gültiger Pose bin ich der Tornächste. Deren Ballzuständigkeit kenne ich nicht.

    pub lowest_other_claiming_player: Option<PlayerNumber>, // current lowest *other* claim received via StateMessage

    pub now: Time,
}

pub struct ReplacementGoalkeeperParameters {
    // move to parameters
    pub election_window: Duration,
    pub no_claim_timeout: Duration,
    pub backoff_step: Duration,
}

pub(crate) fn calculate_available_field_players(world_state: &WorldState) -> Players<bool> {
    let own_number = world_state.robot.player_number;
    let mut available = Players::new(false); // five entries with false created
    /*
    world_state
        .player_states
        .iter()
        .filter(|(number, player_state)| {
            if number == PlayerNumber::One {
                return false;
            }
            if player_state.is_none() {
                return false;
            }

            let known_is_penalized = world
            .state
            .filtered_game_controller_state
            .as_ref()
            .is_some_and(|state| state.penalties[number].is_some());

            if known_is_penalized {
                return false;
            }
            true
        })
        .for_each(|(number, _)| available[number] = true);
    */
    for (number, player_state) in world_state.player_states.iter() {
        if number == PlayerNumber::One {
            continue;
        }

        let known_is_penalized = world_state
            .filtered_game_controller_state
            .as_ref()
            .is_some_and(|state| state.penalties[number].is_some());

        if known_is_penalized {
            continue;
        }

        available[number] = if number == own_number {
            world_state.robot.primary_state != PrimaryState::Penalized // Unnötig?
        } else {
            player_state.is_some()
        };
    }

    available 
}

pub(crate) fn calculate_available_field_players_ole(world_state: &WorldState) -> Vec<PlayerNumber> {
    let own_number = world_state.robot.player_number;
    let mut available = Players::new(false); // five entries with false created

    world_state
        .player_states
        .iter()
        .filter(|(number, player_state)| {
            if *number == PlayerNumber::One {
                return false;
            }
            if player_state.is_none() {
                return false;
            }

            let known_is_penalized = world_state
            .filtered_game_controller_state
            .as_ref()
            .is_some_and(|state| state.penalties[*number].is_some());

            if known_is_penalized {
                return false;
            }
            true
        })
        .map(|(number, _)| number )
        .collect()
        // .for_each(|(number, _)| available[number] = true);
/*
    // anders aufbauen?
    for (number, player_state) in world_state.player_states.iter() {
        if number == PlayerNumber::One {
            continue;
        }

        let known_is_penalized = world
            .state
            .filtered_game_controller_state
            .as_ref()
            .is_some_and(|state| state.penalties[number].is_some());

        if known_is_penalized {
            continue;
        }

        available[number] = if number == own_number {
            world_state.robot.primary_state != PrimaryState::Penalized // Unnötig?
        } else {
            player_state.is_some()
        };
    }

    available 
    */
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ReplacementGoalkeeperState {
    #[default]
    Inactive,
    Candidate,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplacementGoalkeeperCoordinator {
    state: ReplacementGoalkeeperState,
    known_replacement_goalkeeper: Option<PlayerNumber>,
    election_started_at: Option<Time>,
    replacement_needed_since: Option<Time>, // Time since when replacement keeper needed in playing
}

impl ReplacementGoalkeeperCoordinator {
    fn clear_election(&mut self) {
        self.state = ReplacementGoalkeeperState::Inactive;
        self.election_started_at = None;
        self.replacement_needed_since = None;
    }

    fn reset(&mut self) {
        self.clear_election();
        self.known_replacement_goalkeeper = None;
    }

    fn remember_winner(&mut self, winner: PlayerNumber, own_number: PlayerNumber) {
        self.clear_election();
        self.known_replacement_goalkeeper = Some(winner);

        if winner == own_number {
            self.state = ReplacementGoalkeeperState::Active;
        }
    }

    // fn is_eligible_candidate an dieser Stelle einfügen?
    // Zeitmessung und election window handling an dieser einfügen?

    pub fn update(
        &mut self,
        input: ReplacementGoalkeeperInput,
        parameters: &ReplacementGoalkeeperParameters,
    ) {
        let own_number = input.own_player_number;
        let available_count = input
            .available_field_players
            .iter()
            .filter(|(number, available)| *number != PlayerNumber::One && **available)
            .count();

        if !input.regular_goalkeeper_is_penalized || available_count <= 1 {
            self.reset();
            return;
        }

        let own_is_available = own_number != PlayerNumber::One
            && input.available_field_players[own_number]
            && input.primary_state != PrimaryState::Penalized;

        // wenn der replacement keeper nicht mehr available (nicht mehr auf dem Spielfeld) ist, mache reset
        if let Some(winner) = self.known_replacement_goalkeeper {
            // Learners Comment: Wenn k_r_g existiert, weise zu auf winner und führe Block aus
            let winner_is_available = winner != PlayerNumber::One
                && input.available_field_players[winner]
                && (winner != own_number || own_is_available);

            if !winner_is_available {
                self.reset();
            }
        }

        if !own_is_available {
            self.clear_election();
            return;
        }

        // Keep current known_replacement_goalkeeper, when existent
        if let Some(winner) = self.known_replacement_goalkeeper {
            self.remember_winner(winner, own_number);
            return;
        }

        match input.primary_state {
            PrimaryState::Ready => {
                self.clear_election();

                let winner = input
                    .available_field_players
                    .iter()
                    .filter_map(|(number, available)| {
                        (number != PlayerNumber::One && *available).then_some(number)
                    })
                    .min();

                if let Some(winner) = winner {
                    self.remember_winner(winner, own_number);
                }
            }

            PrimaryState::Playing => {
                // Dieser Timer läuft auch ohne eigene Kandidatur.
                let needed_since = *self.replacement_needed_since.get_or_insert(input.now);

                if self.state == ReplacementGoalkeeperState::Candidate
                    && !input.is_eligible_candidate
                {
                    self.state = ReplacementGoalkeeperState::Inactive;
                }

                let other_claim = input.lowest_other_claiming_player;

                // Kleinere verfügbare Nummern beginnen den Fallback früher.
                let backoff_rank = input
                    .available_field_players
                    .iter()
                    .filter(|(number, available)| {
                        **available && *number != PlayerNumber::One && *number < own_number
                    })
                    .count() as u32;

                let fallback_delay =
                    parameters.no_claim_timeout + parameters.backoff_step * backoff_rank;

                let fallback_due = input.now.duration_since(needed_since) >= fallback_delay;

                let should_start_claim = input.is_eligible_candidate
                    && (input.should_claim_first || (other_claim.is_none() && fallback_due));

                if self.state == ReplacementGoalkeeperState::Inactive && should_start_claim {
                    self.state = ReplacementGoalkeeperState::Candidate;
                }

                // Eigenen Claim ohne Netzwerk-Echo einbeziehen.
                let own_claim =
                    (self.state == ReplacementGoalkeeperState::Candidate).then_some(own_number);

                let lowest_claim = other_claim.into_iter().chain(own_claim).min();

                let Some(winner) = lowest_claim else {
                    // Kein Claim: Wahlfenster zurücksetzen.
                    // Der Fallback-Timer läuft weiter.
                    self.election_started_at = None;
                    return;
                };

                // Auch Spieler ohne eigenen Claim beobachten die Wahl.
                let election_start = *self.election_started_at.get_or_insert(input.now);

                let window_elapsed =
                    input.now.duration_since(election_start) >= parameters.election_window;

                if window_elapsed {
                    self.remember_winner(winner, own_number);
                }
            }

            _ => {
                // Insbesondere Set und Stop: keine neue Wahl.
                // Gültige gespeicherte Rollen wurden oben behandelt.
                self.clear_election();
            } /*
              match self.state {
                  ReplacementGoalkeeperState::Inactive => { // bewerbung nur für spielende, geeignete Spieler
                      if input.is_playing && input.is_eligible_candidate {
                          self.state = ReplacementGoalkeeperState::Candidate;
                          self.election_started_at = Some(input.now);
                      }
                  }
                  ReplacementGoalkeeperState::Candidate => {
                      if !input.is_playing || !input.is_eligible_candidate {
                          self.reset();
                          return;
                      }

                      if !input.election_window_elapsed {
                          return;
                      }

                      let lower_claim_exists = input
                          .lowest_claiming_player
                          .is_some_and(|number| number < input.own_player_number);

                      if !lower_claim_exists {
                          self.state = ReplacementGoalkeeperState::Active;
                          self.election_started_at = None;
                      }
                  }
                  ReplacementGoalkeeperState::Active => {}
              }
              */
              // Is there already an active claim?

              // Fallback?
        }
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
            known_replacement_goalkeeper: None,
            election_started_at: None,
            replacement_needed_since: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parameters() -> ReplacementGoalkeeperParameters {
        ReplacementGoalkeeperParameters {
            election_window: Duration::from_millis(500),
            no_claim_timeout: Duration::from_secs(2),
            backoff_step: Duration::from_millis(200),
        }
    }

    fn playing_input(now: Time) -> ReplacementGoalkeeperInput {
        ReplacementGoalkeeperInput {
            regular_goalkeeper_is_penalized: true,
            primary_state: PrimaryState::Playing,
            own_player_number: PlayerNumber::Three,

            available_field_players: Players {
                one: false,
                two: true,
                three: true,
                four: true,
                five: false,
            },

            is_eligible_candidate: true,
            should_claim_first: true,
            lowest_other_claiming_player: None,
            now,
        }
    }

    #[test]
    fn becomes_active_only_after_election_window() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();

        // Bei Wahlbeginn: Candidate, noch kein Gewinner.
        coordinator.update(playing_input(Time::zero()), &parameters);

        assert!(coordinator.claims_role());
        assert!(!coordinator.is_active());
        assert_eq!(coordinator.known_replacement_goalkeeper, None);

        // Nach 499 ms: Das Fenster läuft noch.
        coordinator.update(playing_input(Time::from_nanos(499_000_000)), &parameters);

        assert!(!coordinator.is_active());
        assert_eq!(coordinator.known_replacement_goalkeeper, None);

        // Nach genau 500 ms: Eigener Claim gewinnt ohne Netzwerk-Echo.
        coordinator.update(playing_input(Time::from_nanos(500_000_000)), &parameters);

        assert!(coordinator.is_active());
        assert!(coordinator.claims_role());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Three),
        );
    }

    #[test]
    fn remembers_lower_claim_and_ends_own_candidacy() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();

        // Eigener Spieler 3 und fremder Spieler 2 claimen.
        let mut input = playing_input(Time::zero());
        input.lowest_other_claiming_player = Some(PlayerNumber::Two);
        coordinator.update(input, &parameters);

        assert!(coordinator.claims_role());
        assert!(!coordinator.is_active());

        // Am Wahlende gewinnt Spieler 2.
        let mut input = playing_input(Time::from_nanos(500_000_000));
        input.lowest_other_claiming_player = Some(PlayerNumber::Two);
        coordinator.update(input, &parameters);

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Two),
        );

        // Ohne frischen Claim bleibt das gespeicherte Ergebnis erhalten.
        coordinator.update(playing_input(Time::from_nanos(600_000_000)), &parameters);

        assert!(!coordinator.claims_role());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Two),
        );
    }

    #[test]
    fn clears_replacement_when_regular_goalkeeper_returns() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();

        // Zunächst wird Spieler 3 gewählt.
        coordinator.update(playing_input(Time::zero()), &parameters);
        coordinator.update(playing_input(Time::from_nanos(500_000_000)), &parameters);

        assert!(coordinator.is_active());

        // Anschließend ist Spieler 1 nicht mehr penalized.
        let mut input = playing_input(Time::from_nanos(600_000_000));
        input.regular_goalkeeper_is_penalized = false;
        coordinator.update(input, &parameters);

        assert!(!coordinator.is_active());
        assert!(!coordinator.claims_role());
        assert_eq!(coordinator.known_replacement_goalkeeper, None);
        assert_eq!(coordinator.election_started_at, None);
        assert_eq!(coordinator.replacement_needed_since, None);
    }

    #[test]
    fn later_lower_claim_does_not_replace_active_keeper() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();

        // Spieler 3 gewinnt zunächst die Wahl.
        coordinator.update(playing_input(Time::zero()), &parameters);
        coordinator.update(playing_input(Time::from_nanos(500_000_000)), &parameters);

        assert!(coordinator.is_active());

        // Erst danach claimt Spieler 2.
        let mut input = playing_input(Time::from_nanos(600_000_000));
        input.lowest_other_claiming_player = Some(PlayerNumber::Two);
        coordinator.update(input, &parameters);

        assert!(coordinator.is_active());
        assert!(coordinator.claims_role());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Three),
        );
    }

    #[test]
    fn own_player_is_available_without_own_player_state() {
        let mut world_state = WorldState::default();

        world_state.robot.player_number = PlayerNumber::Three;
        world_state.robot.primary_state = PrimaryState::Playing;

        world_state.filtered_game_controller_state = Some(Default::default());

        world_state.player_states = Players::new(None);

        let available = calculate_available_field_players(&world_state);

        assert_eq!(
            available,
            Players {
                one: false,
                two: false,
                three: true,
                four: false,
                five: false,
            },
        );
    }

}
