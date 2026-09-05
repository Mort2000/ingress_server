# B1 smoke run — `b1-smoke-r1`

- 状态: done (B1 实验基础, `docs/balance/ai-balance-plan.md` section 14 row B1)
- Run directory (untracked): `runs/balance/b1-smoke-r1/`
- Base commit: `7400ecd3044f`
- Reproduce: `cargo run --release -p arena-runner -- balance-b1 runs 7400ecd3044f`
- Date: 2026-09-04

## What this run is

The B1 completion condition made executable: 两配置×两策略×同图换边的小批次可复现 — a small
batch over TWO config variants and TWO policies with same-map faction swap that reproduces
byte-identically. The frozen plan (`arena_runner::balance::smoke::b1_smoke_plan`):

- Stage `smoke`, seed set `development` (ADR-0014, master seed `0x0072326400000300`), first 8 maps.
- Variants: `default` (the untouched `GameConfig::default()`) contrasted with
  `move-budget-240m` (`move_budget_cm` 20 000 -> 24 000, the plan section 7.3 grid's 240 m
  candidate; the single declared changed field is verified at plan-build time and the variant is
  executor-verified to share the identical Map Definition).
- Policies: `greedy-field` (FieldBuilder) and `hybrid` (AdaptiveHybrid), both `m6.0` / `frozen-m6`
  (ADR-0015) — the frozen M6 algorithms driving through the new policy identity layer.
- Pairing (ADR-0014): each comparable unit (map, game_seed, bot_seed, policy pair, bucket policy)
  is played twice per variant — leg 0 `greedy-field` as ENL, leg 1 as RES — and both variants
  reuse the identical unit seeds (common random numbers).
- `Flat(0)` virtual buckets; 12 workers; 2 variants x 8 maps x 2 legs = 32 matches.

## Results (integer raw counts; `runs/balance/b1-smoke-r1/summary.txt`)

```text
total=32 played=32 crashes=0 config_failures=0 replays_captured=16 maps=8 variants=2

## variant default (base)
  units=8 maps=8 legs=16 a_wins=0 b_wins=16 draws=0 crashes=0 config_failures=0
  margin_sum=-861066729349675 outcome_delta_sum_vs_base=0 margin_delta_sum_vs_base=0

## variant move-budget-240m (contrast vs default)
  units=8 maps=8 legs=16 a_wins=0 b_wins=16 draws=0 crashes=0 config_failures=0
  margin_sum=-1047955964309174 outcome_delta_sum_vs_base=0 margin_delta_sum_vs_base=-186889234959499
```

Sanity signals, not conclusions (a 32-match smoke carries no statistical authority — intervals
and clustered analysis are B3/B4):

- Hybrid beats GreedyField 32/32 with GreedyField scoring on most maps — consistent with the
  frozen `runs/m6-r1` evidence of Hybrid dominance; the balance layer reproduces known behavior
  through the new executor.
- The paired margin delta of the 240 m variant against the default on the SAME units is
  `-186 889 234 959 499` score2 (negative = smaller GreedyField-vs-Hybrid margin under the larger
  move budget in this sample). Direction only; the pilot grid decides nothing here.

## Reproducibility evidence

A second full run (into a scratch root) produced:

- `matches.tsv` byte-identical modulo the metadata-only `elapsed_ms` column (32/32 rows);
- `pairs.tsv` byte-identical (integers only, no time columns);
- 16/16 sampled leg-0 replays re-captured.

The archive itself refuses a re-run into an existing experiment directory; historical runs are
never overwritten.

## Manifest summary (`runs/balance/b1-smoke-r1/manifest.txt`)

```text
experiment_id=b1-smoke-r1
stage=smoke
seed_set=development
seed_set_master_seed=0x0072326400000300
seed_set_map_count_frozen=300
maps_used=8
base_variant=default
variants=default:-,move-budget-240m:move_budget_cm
policies=greedy-field=field-builder/m6.0:frozen-m6,hybrid=adaptive-hybrid/m6.0:frozen-m6
matchups=greedy-hybrid=greedy-fieldxhybrid
bucket_policy=flat:0
gates=none
total_matches=32
config_hashes=default=0x9ead052219fa7ff3,move-budget-240m=0x3306905b65b4ce76
schedule_seed=0x3eab1add9979edc0
map_seed_derivation=LE-u64(blake3("ingress-arena-r2-map" || seed_set_tag || master_seed_le64 || k_le32))
game_seed_derivation=LE-u64(blake3("ingress-arena-r2-game" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))
bot_seed_derivation=LE-u64(blake3("ingress-arena-r2-bot" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))
schedule_seed_derivation=LE-u64(blake3("ingress-arena-r2-sched" || experiment_id || master_seed_le64))
pairing=unit=(map, game_seed, bot_seed, policy pair, bucket policy); leg0=policy_a as ENL, leg1=policy_a as RES; variants reuse identical unit seeds
base_commit=7400ecd3044f
workers=12
```

## Artifact content hashes (`runs/balance/b1-smoke-r1/hashes.txt`, plan section 12)

`runs/` is untracked; these blake3 values are the committed record of the run (independent
re-hash of every file matches; `hashes.txt` itself is excluded by construction).

```text
blake3:dd1f414604f66356739b613af06c5c9784deabf5aada9bba2b52e43deee05b61  configs.tsv
blake3:e020d833104b017427d42529346d9b00b9ce2d7ac1f33f124c1a1d846c255840  manifest.txt
blake3:153bed6ea4b9b9a7b7abe58e88783422c3b7ad243d35782b31cc911473125c24  maps/map-153bed6ea4b9b9a7b7abe58e88783422c3b7ad243d35782b31cc911473125c24.bin
blake3:2e0c54509b043362b1cf5718be1bebc57e365716242cbeef3a7525a017294e0e  maps/map-2e0c54509b043362b1cf5718be1bebc57e365716242cbeef3a7525a017294e0e.bin
blake3:669d492bb46969d9d228dbdd18c6c8953c6be0fdef34fa5bcf7298545a579570  maps/map-669d492bb46969d9d228dbdd18c6c8953c6be0fdef34fa5bcf7298545a579570.bin
blake3:7c88fb10cd55ea1fa13b4112892bb9dae40d686a9ebfcd7bd79f9bc9a785bd83  maps/map-7c88fb10cd55ea1fa13b4112892bb9dae40d686a9ebfcd7bd79f9bc9a785bd83.bin
blake3:7ec2ad876fea8613c705814a0a60781f090546c29890039697dff0b9a0b75024  maps/map-7ec2ad876fea8613c705814a0a60781f090546c29890039697dff0b9a0b75024.bin
blake3:8dd2149f2e6edda497560d6aaa85e5d5f8afc13980f9ca2119a4455795e67618  maps/map-8dd2149f2e6edda497560d6aaa85e5d5f8afc13980f9ca2119a4455795e67618.bin
blake3:8e1c482aeeb01eafeb0d73a1b7368ac7330a6ee315134e87375eb922f2e6bb84  maps/map-8e1c482aeeb01eafeb0d73a1b7368ac7330a6ee315134e87375eb922f2e6bb84.bin
blake3:a166c0d31c778a64365cb14f176d7c28bd257758e9f8745aac1efa977a05f82e  maps/map-a166c0d31c778a64365cb14f176d7c28bd257758e9f8745aac1efa977a05f82e.bin
blake3:db7a057ea7c45bdd752b4d0fd352529051c2d418a6602939609d382490c54c05  maps.tsv
blake3:079ab0ab1bbf51103752f7f296797127f4b13f6c7ba75f9500835b2c0f34ff72  matches.tsv
blake3:cb4d719984ed395ebd9d2c8864faab84627bcebbe263853c6a8b6289fc5059f7  pairs.tsv
blake3:5744d00767bf2f88d2240d75975fcadfc2c1c24da55a3ddbc3fccf167919481a  policies.tsv
blake3:b2e028d1690a1e558db7a5ad5d2f78888dbd43b24b6f4c56051d76ce380e6f45  replays/replay-0.iarp
blake3:e937e1293cec9fdb324f5abba96ad35b7f125f64abeaf4a3b8b2e1b8c1b4c298  replays/replay-10.iarp
blake3:a53763da0998c0e11e691542201207aa3b7521b806097f6ad8588b853b7e6673  replays/replay-12.iarp
blake3:b14c9caf05d12a884a048ab6fb42488acc4239a07c5175b4f8343edf5bb5e4c0  replays/replay-14.iarp
blake3:7b6fc2bfff003523eb11aeedc27ee3bbf07bfd51abbe32273a5ccdb581c693d7  replays/replay-16.iarp
blake3:2a309d3bbcbdc612138d0da2c0f8d2736184967a65d5bb4acdcc6904e985ddb1  replays/replay-18.iarp
blake3:5a81057f1e0edb9f377a954c0fd2c7aff6faa9542346f9066886de50cf5cd3fd  replays/replay-2.iarp
blake3:b6ec66ca70ea1fd9d5be005d21c63552de052e841a40846893f4bc7a15948449  replays/replay-20.iarp
blake3:034f251af1b51a8ce87e90ead2874444800de762737d4fcec50a1f205ee5bfd5  replays/replay-22.iarp
blake3:666d50c855334e13077f3523cc71ae443f51570990e2538b0a9367e38b9e8b74  replays/replay-24.iarp
blake3:4f1688e5d8792b6d1059b6e5d7fe3320589bf46365760d5539e7ea5a72c419a2  replays/replay-26.iarp
blake3:9e9961052ae765ddf2d29ba8cb831e40b42e27ec36097f440f70ceeb5bdf0d01  replays/replay-28.iarp
blake3:c5a800d6c9f97a0ae8d4dcbb1aedf609c640800320f5506de2145f9e12084295  replays/replay-30.iarp
blake3:d237123221164dda1fa17b82a99f3e3b16c40a311db9a02a7222c11cd7d5bb82  replays/replay-4.iarp
blake3:96f45b5606521241eed1c65cff4ad988ca26c7d24fe31409cd5a1fd661f625e3  replays/replay-6.iarp
blake3:6b9cb941486d01db47154a8548c37feb2914578e1ae884123d3b16ae94c1dd9b  replays/replay-8.iarp
blake3:829fc3978cdc64352d66e509d5e5e7148cfa7070fc918e7da4daf26afb6d53d2  replays.tsv
blake3:e29c2057f8d57e0cb6c6de3e61b40ec742bc3958819c2eef81482a302f8b964e  report.md
blake3:7b6c550cb3ebcd904349c89ef6cc6c8b982f167c58d16a73891ebf23cf2c0156  summary.txt
```
