// coordinator for decentralized replacement goalkeeper election

use linear_algebra::{Isometry2, Pose2};
use types::world_state::PlayerState; // Nur für die Tests notwendig, trotzdem hier einpflegen?

use hsl_network_messages::{Player, PlayerNumber};
use linear_algebra::point;
use ros_z::time::Time;
use std::time::Duration;
use serde::{Deserialize, Serialize}; // was macht das?
use types::{
    behavior_tree::Status,
    players::Players,
    primary_state::PrimaryState,
    world_state::{self, WorldState},
};

use crate::node::Blackboard;

#[derive(Debug, Clone, Copy)]
pub struct ReplacementGoalkeeperRoleInput {
    pub regular_goalkeeper_is_penalized: bool,
    pub primary_state: PrimaryState,
    pub own_player_number: PlayerNumber,
    pub available_field_players: Players<bool>,
}

pub struct ReplacementGoalkeeperInput {
    pub regular_goalkeeper_is_penalized: bool,
    pub primary_state: PrimaryState,
    pub own_player_number: PlayerNumber,

    pub available_field_players: Players<bool>, // Players on field, abgleitet aus gültigen empfangenen PlayerStates
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

// ToDo: Fehlender-GameController Zustand ergibt hier false, wodurch eine bestehende Rolle zurückgesetzt wird. In Zukunft ändern!
pub fn refresh_replacement_goalkeeper_role(
    blackboard: &mut Blackboard,
) -> Status {
    let world_state = &blackboard.world_state;

    let input = ReplacementGoalkeeperRoleInput {
        regular_goalkeeper_is_penalized: world_state
            .filtered_game_controller_state
            .as_ref()
            .is_some_and(|state| {
                state.penalties[PlayerNumber::One].is_some() // TODO Parametrisieren der PlayerNumber::One
            }),
        primary_state: world_state.robot.primary_state,
        own_player_number: world_state.robot.player_number,
        available_field_players:
            calculate_available_field_players(world_state),
    };

    blackboard
        .replacement_goalkeeper_coordinator
        .refresh_role_status(input);

    Status::Success
}

/// todo sollte umbenannt werden is is_closest_to_goal oder so was in der art
/// Returns whether this robot should claim first based on goal distance.
/// Equal distances are resolved in favor of the lower player number.
pub(crate) fn should_claim_first(
    blackboard: &Blackboard,
    available_field_players: &Players<bool>,
) -> bool {
    let world_state = &blackboard.world_state;
    let own_number = world_state.robot.player_number;

    if own_number == PlayerNumber::One
        || !available_field_players[own_number]
    {
        return false;
    }

    let Some(ground_to_field) =
        world_state.robot.ground_to_field
    else {
        return false;
    };

    let own_goal = point!(
        -blackboard.field_dimensions.length / 2.0,
        0.0
    );

    let own_distance_squared =
        (ground_to_field.translation() - own_goal).norm_squared();

    for (number, player_state) in world_state.player_states.iter() {
        if number == PlayerNumber::One
            || number == own_number
            || !available_field_players[number]
        {
            continue;
        }

        let Some(player_state) = player_state else {
            continue;
        };

        let other_distance_squared =
            (player_state.pose.position() - own_goal).norm_squared();

        if other_distance_squared < own_distance_squared
            || (other_distance_squared == own_distance_squared
                && number < own_number)
        {
            return false;
        }
    }

    true
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

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq,
    Serialize, Deserialize, ros_z::Message,
)]
enum ReplacementGoalkeeperState {
    #[default]
    Inactive,
    Candidate,
    Active,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq,
    Serialize, Deserialize, ros_z::Message,
)]
pub struct ReplacementGoalkeeperCoordinator {
    state: ReplacementGoalkeeperState,
    known_replacement_goalkeeper: Option<PlayerNumber>,
    election_started_at: Option<Time>,
    replacement_needed_since: Option<Time>,
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

    /// Validates the remembered replacement goalkeeper and clears invalid roles
    /// or candidacies. Returns whether keeper selection may proceed.
    /// Does not start or advance an election.
    pub fn refresh_role_status(
        &mut self,
        input: ReplacementGoalkeeperRoleInput,
    ) -> bool {
        let own_number = input.own_player_number;

        // Warum brauche ich hier den Filter?
        let available_count = input
            .available_field_players
            .iter()
            .filter(|(number, available)| {
                *number != PlayerNumber::One && **available
            })
            .count();
        
        if !input.regular_goalkeeper_is_penalized || available_count <= 1 {
            self.reset();
            return false;
        }

        // Notwendig so elaboriert? 
        let own_is_available =
            own_number != PlayerNumber::One // TODO parameterize
                && input.available_field_players[own_number]
                && input.primary_state != PrimaryState::Penalized;

        if let Some(winner) = self.known_replacement_goalkeeper {
            let winner_is_available =
                winner != PlayerNumber::One
                    && input.available_field_players[winner]
                    && (winner != own_number || own_is_available);

            if !winner_is_available {
                self.reset();
            }
        }

        if !own_is_available {
            self.clear_election();
            return false;
        }

        // Eine gültige Besetzung bleibt bestehen, aber muss man das so machen?
        if let Some(winner) = self.known_replacement_goalkeeper {
            self.remember_winner(winner, own_number);
            return false;
        }

        if input.primary_state != PrimaryState::Playing {
            self.clear_election();
        }

        matches!(
            input.primary_state,
            PrimaryState::Ready | PrimaryState::Playing
        )
        // true: Auswahl darf weiter geprüft werden.
        // false: In diesem Update keine Auswahl fortsetzen.
    }

    // fn is_eligible_candidate an dieser Stelle einfügen?
    // Zeitmessung und election window handling an dieser einfügen?

    pub fn update(
        &mut self,
        input: ReplacementGoalkeeperInput,
        parameters: &ReplacementGoalkeeperParameters,
    ) {
        let role_input = ReplacementGoalkeeperRoleInput {
            regular_goalkeeper_is_penalized:
                input.regular_goalkeeper_is_penalized,
            primary_state: input.primary_state,
            own_player_number: input.own_player_number,
            available_field_players: input.available_field_players,
        };

        if !self.refresh_role_status(role_input) {
            return;
        }

        let own_number = input.own_player_number;

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

    fn goal_distance_blackboard() -> Blackboard {
    let mut blackboard = Blackboard {
        field_dimensions: Default::default(),
        parameters: Default::default(),
        world_state: WorldState::default(),
        replacement_goalkeeper_coordinator: Default::default(),

        path_obstacles_output: Vec::new(),
        time_since_last_switch: Duration::ZERO,
        direction_difference: 0.0,
        voronoi_inputs: Vec::new(),

        ball: None,
        visual_kick_ball_position: None,
        last_ball: None,
        last_close_enough_to_kick: false,
        last_kick_target: None,
        last_motion_command: Default::default(),
        last_motion_switch_time: Time::zero(),
        last_motion_type: None,
        last_sent_game_controller_return_message_time: None,
        last_sent_hsl_message_time: None,
        last_closest_to_ball: false,
        closest_to_ball_entered_area_since: None,
        closest_to_ball_left_area_since: None,

        is_injected_motion_command: false,
        walk_position: None,
        body_motion: None,
        head_motion: None,
        voronoi_map: None,
    };

    blackboard.field_dimensions.length = 10.0;
    blackboard.world_state.robot.player_number = PlayerNumber::Three;
    blackboard.world_state.robot.primary_state = PrimaryState::Playing;
    blackboard.world_state.robot.ground_to_field =
        Some(Isometry2::from(point!(-4.0, 0.0)));

    blackboard.world_state.player_states.two = Some(PlayerState {
        pose: Pose2::new(point!(-3.0, 0.0), 0.0),
        ..Default::default()
    });

    blackboard
}

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

    #[test]
    fn candidate_withdraws_in_stop() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();
        let input = playing_input(Time::zero());

        let stop_input = ReplacementGoalkeeperRoleInput {
            regular_goalkeeper_is_penalized:
                input.regular_goalkeeper_is_penalized,
            primary_state: PrimaryState::Stop,
            own_player_number: input.own_player_number,
            available_field_players: input.available_field_players,
        };

        coordinator.update(input, &parameters);

        assert_eq!(
            coordinator.state,
            ReplacementGoalkeeperState::Candidate,
        );
        assert!(coordinator.claims_role());
        assert!(coordinator.election_started_at.is_some());
        assert!(coordinator.replacement_needed_since.is_some());

        let selection_allowed = coordinator.refresh_role_status(stop_input);

        assert!(!selection_allowed);
        assert_eq!(
            coordinator.state,
            ReplacementGoalkeeperState::Inactive,
        );
        assert!(!coordinator.claims_role());
        assert_eq!(coordinator.known_replacement_goalkeeper, None);
        assert_eq!(coordinator.election_started_at, None);
        assert_eq!(coordinator.replacement_needed_since, None);
    }

    #[test]
    fn active_replacement_stays_active_in_stop() {
        let mut coordinator = ReplacementGoalkeeperCoordinator::default();
        let parameters = parameters();
        let input = playing_input(Time::zero());

        let stop_input = ReplacementGoalkeeperRoleInput {
            regular_goalkeeper_is_penalized:
                input.regular_goalkeeper_is_penalized,
            primary_state: PrimaryState::Stop,
            own_player_number: input.own_player_number,
            available_field_players: input.available_field_players,
        };

        // Wahl beginnen und nach 500 ms gewinnen.
        coordinator.update(input, &parameters);
        coordinator.update(
            playing_input(Time::from_nanos(500_000_000)), // duration of election window elapsed
            &parameters,
        );

        assert!(coordinator.is_active());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Three),
        );

        let selection_allowed = coordinator.refresh_role_status(stop_input);

        assert!(!selection_allowed);
        assert!(coordinator.is_active());
        assert!(coordinator.claims_role());
        assert_eq!(
            coordinator.known_replacement_goalkeeper,
            Some(PlayerNumber::Three),
        );
        assert_eq!(coordinator.election_started_at, None);
        assert_eq!(coordinator.replacement_needed_since, None);
    }

    #[test]
    fn should_claim_first_when_own_player_is_closest_to_goal() {
        let blackboard = goal_distance_blackboard();
        let available =
            calculate_available_field_players(&blackboard.world_state);

        // Eigene lokale Pose reicht; kein eigenes Netzwerk-Echo nötig.
        assert!(blackboard.world_state.player_states.three.is_none());
        assert!(should_claim_first(&blackboard, &available));
    }

    #[test]
    fn should_claim_first_is_false_when_other_player_is_closer() {
        let mut blackboard = goal_distance_blackboard();
        blackboard.world_state.player_states.two.as_mut().unwrap().pose =
            Pose2::new(point!(-4.5, 0.0), 0.0);

        let available =
            calculate_available_field_players(&blackboard.world_state);

        assert!(!should_claim_first(&blackboard, &available));
    }

    #[test]
    fn should_claim_first_ignores_unavailable_closer_player() {
        let mut blackboard = goal_distance_blackboard();
        blackboard.world_state.player_states.two.as_mut().unwrap().pose =
            Pose2::new(point!(-4.5, 0.0), 0.0);

        let mut available =
            calculate_available_field_players(&blackboard.world_state);
        available.two = false;

        assert!(should_claim_first(&blackboard, &available));
    }

    #[test]
    fn should_claim_first_breaks_ties_by_player_number() {
        for (other_number, expected) in [
            (PlayerNumber::Two, false),
            (PlayerNumber::Four, true),
        ] {
            let mut blackboard = goal_distance_blackboard();
            blackboard.world_state.player_states = Players::new(None);

            // Gleiche Position bedeutet exakt gleiche Entfernung.
            blackboard.world_state.player_states[other_number] =
                Some(PlayerState {
                    pose: Pose2::new(point!(-4.0, 0.0), 0.0),
                    ..Default::default()
                });

            let available =
                calculate_available_field_players(&blackboard.world_state);

            assert_eq!(
                should_claim_first(&blackboard, &available),
                expected,
                "tie with player {other_number:?}",
            );
        }
    }

    #[test]
    fn should_claim_first_is_false_without_own_pose() {
        let mut blackboard = goal_distance_blackboard();
        blackboard.world_state.robot.ground_to_field = None;

        let available =
            calculate_available_field_players(&blackboard.world_state);

        assert!(!should_claim_first(&blackboard, &available));
    }
    
}
