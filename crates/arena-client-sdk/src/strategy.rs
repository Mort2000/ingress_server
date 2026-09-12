//! Optional owned-observation strategy seam. Live submissions are forwarded
//! immediately; offline budgets are explicit inputs, never a hidden clock.
use crate::{ClientBot, RoundContext, SdkError, SpawnContext};
use arena_core::{
    harness::{AgentSubmission, RoundStrategy},
    observe::Observation,
    types::{AgentId, Command, Coord},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    pub agent: AgentId,
    pub task: String,
    pub target: String,
    pub value: i64,
    /// Strategy-declared elapsed planning time, not a server measurement.
    pub elapsed_us: u64,
}

/// A strategy's legal round input and output boundary. Commit is explicit;
/// submitting the last own Agent also closes it, as in `RoundContext`.
pub trait Decision {
    fn remaining_ms(&mut self) -> Result<u64, SdkError>;
    fn submit(&mut self, agent: AgentId, commands: Vec<Command>) -> Result<(), SdkError>;
    fn commit(&mut self) -> Result<(), SdkError>;
    fn trace(&mut self, trace: Trace) -> Result<(), SdkError>;
}

pub trait Strategy: Send {
    fn decide(&mut self, obs: &Observation, turn: &mut dyn Decision) -> Result<(), SdkError>;
    fn on_feedback(&mut self, _feedback: &crate::SubmissionFeedback) {}
    fn on_diagnostics(&mut self, _round: u32, _budgets: &[u64], _trace: &[Trace]) {}
    fn on_timing(&mut self, _timing: &crate::RoundTiming) {}
}

fn push_trace(traces: &mut Vec<Trace>, trace: Trace, roster: &[AgentId]) -> Result<(), SdkError> {
    if !roster.contains(&trace.agent)
        || traces.len() >= 1024
        || trace.task.len() > 1024
        || trace.target.len() > 1024
    {
        return Err(SdkError::Protocol(
            "trace exceeds own-roster or diagnostic limits".into(),
        ));
    }
    traces.push(trace);
    Ok(())
}

/// Wrap a strategy constructed by `run`'s factory. Spawn choices are supplied
/// once by that factory using the received config, faction and frozen map.
pub struct LiveStrategy<S> {
    pub strategy: S,
    pub spawns: Vec<(AgentId, Coord)>,
    /// First adapter error; live clients should log it or end their workflow.
    pub error: Option<String>,
}

impl<S> LiveStrategy<S> {
    pub fn new(strategy: S, spawns: Vec<(AgentId, Coord)>) -> Self {
        Self {
            strategy,
            spawns,
            error: None,
        }
    }
}

struct LiveDecision<'a> {
    ctx: RoundContext<'a>,
    roster: Vec<AgentId>,
    budgets: Vec<u64>,
    traces: Vec<Trace>,
}
impl Decision for LiveDecision<'_> {
    fn remaining_ms(&mut self) -> Result<u64, SdkError> {
        if self.budgets.len() >= 65536 {
            return Err(SdkError::Protocol(
                "budget read diagnostic limit exceeded".into(),
            ));
        }
        let value = self.ctx.remaining_ms();
        self.budgets.push(value);
        Ok(value)
    }
    fn submit(&mut self, agent: AgentId, commands: Vec<Command>) -> Result<(), SdkError> {
        self.ctx.submit(agent, commands).map(|_| ())
    }
    fn commit(&mut self) -> Result<(), SdkError> {
        self.ctx.commit()
    }
    fn trace(&mut self, trace: Trace) -> Result<(), SdkError> {
        push_trace(&mut self.traces, trace, &self.roster)
    }
}
impl<S: Strategy> ClientBot for LiveStrategy<S> {
    fn choose_spawns(&mut self, _: &SpawnContext<'_>) -> Vec<(AgentId, Coord)> {
        self.spawns.clone()
    }
    fn on_round(&mut self, ctx: RoundContext<'_>) {
        let result = (|| {
            let obs = ctx.observation()?.to_observation()?;
            let mut turn = LiveDecision {
                ctx,
                roster: obs.own_agents.iter().map(|a| a.id).collect(),
                budgets: vec![],
                traces: vec![],
            };
            let result = self.strategy.decide(&obs, &mut turn);
            self.strategy
                .on_diagnostics(obs.round, &turn.budgets, &turn.traces);
            result
        })();
        if let Err(error) = result {
            eprintln!("arena strategy error: {error}");
            self.error = Some(error.to_string());
        }
    }
    fn on_submission_feedback(&mut self, feedback: &crate::SubmissionFeedback) {
        self.strategy.on_feedback(feedback);
    }
    fn on_round_timing(&mut self, timing: &crate::RoundTiming) {
        self.strategy.on_timing(timing);
    }
}

/// Fixed-observation local counterfactual. This is not a new full match.
/// `bucket` is an injected functional-test input, not measured arrival time.
pub struct OfflineRound {
    remaining: Vec<AgentId>,
    roster: Vec<AgentId>,
    budgets: std::collections::VecDeque<u64>,
    reads: Vec<u64>,
    submissions: Vec<AgentSubmission>,
    traces: Vec<Trace>,
    closed: bool,
    bucket: u64,
}
impl OfflineRound {
    pub fn new(obs: &Observation, budgets: Vec<u64>, bucket: u64) -> Self {
        let roster: Vec<_> = obs.own_agents.iter().map(|a| a.id).collect();
        Self {
            remaining: roster.clone(),
            roster,
            budgets: budgets.into(),
            reads: vec![],
            submissions: vec![],
            traces: vec![],
            closed: false,
            bucket,
        }
    }
    pub fn submissions(&self) -> &[AgentSubmission] {
        &self.submissions
    }
    pub fn budget_reads(&self) -> &[u64] {
        &self.reads
    }
    pub fn traces(&self) -> &[Trace] {
        &self.traces
    }
    pub fn is_committed(&self) -> bool {
        self.closed
    }
}
impl Decision for OfflineRound {
    fn remaining_ms(&mut self) -> Result<u64, SdkError> {
        let value = self
            .budgets
            .pop_front()
            .ok_or_else(|| SdkError::Protocol("injected budget sequence exhausted".into()))?;
        self.reads.push(value);
        Ok(value)
    }
    fn submit(&mut self, agent: AgentId, commands: Vec<Command>) -> Result<(), SdkError> {
        if self.closed {
            return Err(SdkError::ContextClosed);
        }
        self.submissions.push(AgentSubmission {
            agent_id: agent,
            commands,
            bucket: self.bucket,
        });
        self.remaining.retain(|a| *a != agent);
        self.closed = self.remaining.is_empty();
        Ok(())
    }
    fn commit(&mut self) -> Result<(), SdkError> {
        self.closed = true;
        Ok(())
    }
    fn trace(&mut self, trace: Trace) -> Result<(), SdkError> {
        push_trace(&mut self.traces, trace, &self.roster)
    }
}

/// Both factions can use this adapter in the Core Harness and re-decide
/// against the newly settled state. Budget inputs are supplied per round.
pub struct HarnessStrategy<S, B> {
    pub strategy: S,
    pub budgets: B,
    pub bucket: u64,
}
impl<S: Strategy, B: FnMut(u32) -> Vec<u64>> RoundStrategy for HarnessStrategy<S, B> {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let mut turn = OfflineRound::new(obs, (self.budgets)(obs.round), self.bucket);
        self.strategy
            .decide(obs, &mut turn)
            .expect("offline strategy failed; fixture is not valid");
        self.strategy
            .on_diagnostics(obs.round, turn.budget_reads(), turn.traces());
        turn.submissions
    }
}
