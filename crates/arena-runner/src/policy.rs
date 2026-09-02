//! Virtual bucket policy plumbing (D56, R-RESOLVE-03): the balance default
//! puts every stream of both factions into bucket 0 (arrival = collection
//! order, parity alternation per D19/D20); an explicit `Factions` policy
//! pins one bucket per faction to exercise cross-bucket first-mover
//! semantics. Bots themselves always state bucket 0 — the policy is a
//! batch concern applied AFTER the bot returns, and the harness's D18
//! receive-time derivation stays untouched.

use arena_core::harness::{AgentSubmission, RoundStrategy};
use arena_core::observe::Observation;
use arena_core::types::Faction;

/// A batch-wide virtual bucket assignment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BucketPolicy {
    /// Every stream of both factions states this bucket (the balance
    /// default; bucket 0 = arrival order is the collection order).
    Flat(u64),
    /// One bucket per faction: every ENL stream precedes every RES stream
    /// (`enl < res`) in every round — the explicit cross-bucket case.
    Factions { enl: u64, res: u64 },
}

impl BucketPolicy {
    /// The bucket a faction's streams are rewritten to.
    pub fn bucket_for(self, faction: Faction) -> u64 {
        match self {
            BucketPolicy::Flat(bucket) => bucket,
            BucketPolicy::Factions { enl, res } => match faction {
                Faction::Enl => enl,
                Faction::Res => res,
            },
        }
    }

    /// Stable text tag for manifests and TSV rows.
    pub fn tag(self) -> String {
        match self {
            BucketPolicy::Flat(bucket) => format!("flat:{bucket}"),
            BucketPolicy::Factions { enl, res } => format!("factions:{enl},{res}"),
        }
    }

    /// Parses [`BucketPolicy::tag`] back (report round-trips).
    pub fn from_tag(tag: &str) -> Option<BucketPolicy> {
        if let Some(rest) = tag.strip_prefix("flat:") {
            return rest.parse::<u64>().ok().map(BucketPolicy::Flat);
        }
        let rest = tag.strip_prefix("factions:")?;
        let (enl, res) = rest.split_once(',')?;
        Some(BucketPolicy::Factions {
            enl: enl.parse().ok()?,
            res: res.parse().ok()?,
        })
    }
}

/// Wraps one faction's strategy and rewrites every returned submission's
/// bucket to the policy bucket for that faction. Nothing else changes —
/// the strategy's returned order stays the faction's arrival order.
pub struct PolicyStrategy {
    inner: Box<dyn RoundStrategy>,
    bucket: u64,
}

impl PolicyStrategy {
    pub fn new(inner: Box<dyn RoundStrategy>, faction: Faction, policy: BucketPolicy) -> Self {
        PolicyStrategy {
            inner,
            bucket: policy.bucket_for(faction),
        }
    }
}

impl RoundStrategy for PolicyStrategy {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let mut submissions = self.inner.submit_round(obs);
        for submission in &mut submissions {
            submission.bucket = self.bucket;
        }
        submissions
    }
}
