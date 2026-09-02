//! The participant loop: connect, HELLO, spawn window, then the round
//! loop until GAME_END. The SDK stamps the version triple itself
//! (participants cannot mis-declare versions through the official path).

use std::path::PathBuf;
use std::time::Duration;

use arena_core::types::{AgentId, Coord, Faction};
use arena_protocol::decode::{
    END_CLIENT_FORFEIT, END_DRAW, END_ENL_WIN, END_RES_WIN, END_SETUP_ERROR, END_SPAWN_FORFEIT,
    Envelope, HELLO_OK, MatchInfo,
};

use crate::SdkError;
use crate::bot::ClientBot;
use crate::conn::Connection;
use crate::round::Session;

/// Connection parameters for [`run`].
#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub socket_path: PathBuf,
    pub client_name: String,
    pub client_version: String,
    /// Per-read timeout while waiting for server frames (the loop keeps
    /// waiting until an error or GAME_END; this only bounds each read).
    pub read_timeout: Duration,
}

impl ConnectOptions {
    pub fn new(socket_path: impl Into<PathBuf>, client_name: impl Into<String>) -> Self {
        ConnectOptions {
            socket_path: socket_path.into(),
            client_name: client_name.into(),
            client_version: concat!("sdk-", env!("CARGO_PKG_VERSION")).into(),
            read_timeout: Duration::from_secs(30),
        }
    }
}

/// How the match ended for this client.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunSummary {
    Normal {
        outcome: u8,
        rounds_completed: u32,
        score_enl: u128,
        score_res: u128,
    },
    /// The server ended the match technically; participation is over and
    /// no reconnect is attempted (P3-TIMEOUT).
    Aborted { outcome: u8, detail: String },
}

/// Runs one whole match as one client. Blocks until GAME_END (or an
/// error). The participant logic is CONSTRUCTED after SERVER_HELLO and
/// INITIAL_SPAWN_START: `make_bot` receives the assigned faction, the
/// server's rule constants, and the complete frozen map (the spawn-phase
/// visibility of D54) — bots derive their RNG keys and any captured plan
/// from all three.
pub fn run<B: ClientBot>(
    make_bot: impl FnOnce(
        Faction,
        &arena_core::config::GameConfig,
        &arena_core::map::MapDefinition,
    ) -> B,
    opts: ConnectOptions,
) -> Result<RunSummary, SdkError> {
    let mut conn = Connection::connect(&opts.socket_path)?;
    conn.send(&Envelope::ClientHello {
        client_name: opts.client_name,
        client_version: opts.client_version,
        protocol_version: arena_protocol::PROTOCOL_VERSION,
        public_snapshot_version: arena_protocol::PUBLIC_SNAPSHOT_VERSION,
        private_snapshot_version: arena_protocol::PRIVATE_SNAPSHOT_VERSION,
    })?;

    // SERVER_HELLO
    let (status, faction, match_id) = match conn.expect_envelope(opts.read_timeout)? {
        Envelope::ServerHello {
            status,
            assigned_faction,
            match_id,
        } => (status, assigned_faction, match_id),
        other => {
            return Err(SdkError::Unexpected(format!(
                "expected SERVER_HELLO, got {other:?}"
            )));
        }
    };
    if status != HELLO_OK {
        return Err(SdkError::VersionRejected(format!("status={status}")));
    }

    // INITIAL_SPAWN_START
    let info: MatchInfo = match conn.expect_envelope(opts.read_timeout)? {
        Envelope::InitialSpawnStart { info, .. } => info,
        other => {
            return Err(SdkError::Unexpected(format!(
                "expected INITIAL_SPAWN_START, got {other:?}"
            )));
        }
    };
    let config = info.to_core();
    config
        .validate()
        .map_err(|error| SdkError::Unexpected(format!("server config invalid: {error:?}")))?;

    let session = Session::new(&match_id, faction, config)?;
    session.phase.on_initial_spawn_start();

    // The frozen map is readable exactly in the spawn phase (D54): capture
    // it once for the participant and for the spawn choice.
    let map = session.spawn_context()?.map_view()?.to_map_definition()?;
    let mut bot = make_bot(faction, &session.config, &map);

    // Spawn phase: the bot sees the complete frozen map, then submits the
    // whole set in one frame.
    let positions: Vec<(AgentId, Coord)> = {
        let ctx = session.spawn_context()?;
        bot.choose_spawns(&ctx)
    };
    conn.send(&Envelope::SubmitInitialSpawns {
        positions: positions
            .into_iter()
            .map(|(agent_id, position)| arena_protocol::decode::SpawnEntry { agent_id, position })
            .collect(),
    })?;
    match conn.expect_envelope(opts.read_timeout)? {
        Envelope::InitialSpawnResult { status: 0, .. } => {}
        Envelope::InitialSpawnResult { status, detail } => {
            return Err(SdkError::SpawnForfeit(format!(
                "status={status} detail={detail}"
            )));
        }
        other => {
            return Err(SdkError::Unexpected(format!(
                "expected INITIAL_SPAWN_RESULT, got {other:?}"
            )));
        }
    }

    // Round loop.
    loop {
        let envelope = conn.expect_envelope(opts.read_timeout)?;
        match envelope {
            Envelope::RoundStartSignal => {
                session.phase.on_round_start();
                let round = session.public.read()?.current_round;
                let ctx = session.round_context(&mut conn, round)?;
                bot.on_round(ctx);
                // The context is dropped here at the latest; late submits
                // are refused by the server anyway.
            }
            Envelope::RoundClosed { .. } => {
                session.phase.on_round_closed();
            }
            Envelope::SubmissionResult {
                agent_id,
                status,
                detail,
                ..
            } => session.record_result(agent_id, status, detail),
            Envelope::GameEnd {
                outcome,
                detail,
                score_enl,
                score_res,
                rounds_completed,
            } => {
                session.phase.on_round_closed();
                return Ok(match outcome {
                    END_ENL_WIN | END_RES_WIN | END_DRAW => RunSummary::Normal {
                        outcome,
                        rounds_completed,
                        score_enl,
                        score_res,
                    },
                    END_SPAWN_FORFEIT | END_CLIENT_FORFEIT | END_SETUP_ERROR => {
                        RunSummary::Aborted { outcome, detail }
                    }
                    _ => RunSummary::Aborted { outcome, detail },
                });
            }
            other => {
                return Err(SdkError::Unexpected(format!(
                    "unexpected envelope in the round loop: {other:?}"
                )));
            }
        }
    }
}
