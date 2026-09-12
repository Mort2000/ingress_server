export type Faction = "enl" | "res";
export type View = Faction | "god" | "public";
export type Point = [number, number];
export type AgentRow = [id: number, x: number, y: number, supply: number];
export interface Command {
  name: string;
  params: string;
  major: boolean;
  status: "ok" | "drop" | "skipped";
  detail: string;
  cost_cm?: number;
  from?: Point;
  to?: Point;
}
export interface Stream {
  slot: string;
  reason: string;
  order: number | null;
  bucket: string | null;
  seq: number | null;
  cmds: Command[];
}
export interface ReplayEvent {
  k: number;
  f?: number;
  a?: number;
  p?: number;
  l?: number[];
  t?: number[];
  d?: number;
  h?: number;
  n?: number;
  pos?: Point;
}
export interface Round {
  r: number;
  score2: [string, string];
  delta2: [string, string];
  portals: [number, number, number][];
  links: [number, number, number][];
  fields: [number, number, number, number, number][];
  events: ReplayEvent[];
  hash: string;
}
export interface Replay {
  header: {
    game_seed: string;
    total_rounds: number;
    rounds_completed: number;
    versions: { protocol: number };
    identity?: {
      execution_id: string;
      participants: ParticipantIdentity[];
    } | null;
    config: Record<string, number>;
    map: { portals: [number, number, number][] };
    spawns: Record<Faction, [number, number, number][]>;
    outcome: {
      kind: string;
      result?: string;
      outcome_tag?: number;
      final_hash?: string;
      last_hash?: string;
      final_score2: [string, string];
    };
    checkpoints: number[];
  };
  rounds: Round[];
  agents: Record<Faction, AgentRow[][]>;
  keys: Record<Faction, Record<string, Record<string, number>>[]>;
  commands: Record<Faction, Record<string, Stream>[]>;
}
export interface Agent {
  faction: Faction;
  row: AgentRow;
}
export interface ParticipantIdentity {
  registered_id: string;
  display_name: string;
  registered_version: string;
  binary_hash: string;
  client_name: string;
  client_version: string;
  bot_version: string;
  sdk_version: string;
  binding_verified: boolean;
}
export const factions: Faction[] = ["enl", "res"];
export const colors = {
  enl: "#41ff7e",
  res: "#3fb6ff",
  neutral: "#93a4b8",
  key: "#ffc857",
};
export const score = (value: string) => {
  const n = BigInt(value);
  return (n / 2n).toLocaleString("en-US") + (n % 2n ? ".5" : "");
};
export class ReplayModel {
  round = 0;
  view: View = "god";
  selected: number | null = null;
  hoverPortal: number | null = null;
  constructor(readonly data: Replay) {
    this.selected = this.agents[0]?.row[0] ?? null;
  }
  get visibleFactions(): Faction[] {
    return this.view === "god"
      ? factions
      : this.view === "public"
        ? []
        : [this.view];
  }
  get agents(): Agent[] {
    return this.visibleFactions.flatMap((faction) =>
      (this.round
        ? this.data.agents[faction][this.round - 1]
        : this.data.header.spawns[faction].map(
            (row) =>
              [...row, this.data.header.config.initial_supply] as AgentRow,
          )
      ).map((row) => ({ faction, row })),
    );
  }
  get agent(): Agent | undefined {
    return this.agents.find((a) => a.row[0] === this.selected);
  }
  get snapshot(): Round {
    return (
      this.data.rounds[this.round - 1] ?? {
        r: 0,
        score2: ["0", "0"],
        delta2: ["0", "0"],
        portals: this.data.header.map.portals.map((p) => [p[0], 0, 0]),
        links: [],
        fields: [],
        events: [],
        hash: "",
      }
    );
  }
  keys(agent: Agent): Record<string, number> {
    return this.data.keys[agent.faction][this.round - 1]?.[agent.row[0]] ?? {};
  }
  stream(agent: Agent, round = this.round): Stream | undefined {
    return this.data.commands[agent.faction][round - 1]?.[agent.row[0]];
  }
  holders(portal: number): Agent[] {
    return this.agents.filter((a) => (this.keys(a)[portal] ?? 0) > 0);
  }
  seek(round: number) {
    this.round = Math.max(
      0,
      Math.min(this.data.rounds.length, Math.round(round)),
    );
    this.hoverPortal = null;
  }
  setView(view: View) {
    this.view = view;
    this.hoverPortal = null;
    if (!this.agent) this.selected = this.agents[0]?.row[0] ?? null;
  }
  get movedCm(): number {
    return this.agent
      ? (this.stream(this.agent)?.cmds ?? []).reduce(
          (sum, cmd) => sum + (cmd.status === "ok" ? (cmd.cost_cm ?? 0) : 0),
          0,
        )
      : 0;
  }
  movements(agent: Agent): { round: number; from: Point; to: Point }[] {
    const segments = [];
    for (let r = 1; r <= this.round; r++)
      for (const cmd of this.stream(agent, r)?.cmds ?? []) {
        if (cmd.status === "ok" && cmd.from && cmd.to)
          segments.push({ round: r, from: cmd.from, to: cmd.to });
      }
    return segments;
  }
}
