# About the analyzer

Why this crate treats a file Read as a skill call, and why the window looks the way it does.

It started as a way to read a personal fork of Lauren Tan's pstack plugin. The fork is [guide-to-the-code](https://github.com/apersic/guide-to-the-code). The analyzer now loads any installed Cursor plugin.

## The log is incomplete

Cursor JSONL transcripts record `role`, `message`, `tool_use`, and `turn_ended`. They do not record skill invocations, parent model, or billed tokens. A plugin skill is a file the agent is told to Read. The only durable signal is that Read, plus what the same thread does next.

So the projector does not wait for a hook that does not exist. A classified `Read` of `SKILL.md` or a playbook is the call. HIT and MISS are derived.

## HIT means the Read earned its place

A Read that is never used is noise. A citation with no Read is a claim. The rules sit on later evidence in the same thread:

- A `TodoWrite` step that names the file and is not `skip:` is use.
- A citation followed by `Edit`, `Shell`, or `Spawn` is use.
- A `references/` Read after the skill itself is use.

A settled thread that Read the file and never did those things is a MISS. An open thread that only Read it stays UNKNOWN. That gap is why the LIVE badge exists. The courier can walk a path that is still being written.

## One snapshot, three folds

There is no second store. `Scanner` tails `~/.cursor/projects/**/agent-transcripts/**/*.jsonl`. `Query.scope` folds All, one workspace, or one chat from that snapshot. Replaying the same bytes yields the same digest.

The graph is that fold. Nodes are the prompt, the docs that were touched, and the models that were spawned. Edges are the latest Read path plus playbook-to-skill mentions that the same thread also Read. Lane is hue. HIT/MISS is the halo.

A `chat` node at the origin keeps the path attached to a person-sized object. Without it the first hop starts in empty space.

## Live without a jump

A full relayout every poll throws the camera. Three choices keep the picture still:

- Prefer an open root thread over a newer settled one, so the courier stays on the turn you are watching.
- Warm-start layout from the previous positions.
- Fit the camera once on a valid rect. `locked` refits every frame. The window starts fitted and unlocked, because a lock you did not ask for fights the first pan.

The first frame of a `CentralPanel` can be tiny. Fitting that rect pins a useless zoom. `needs_fit` waits until width and height are both above 80.

## Two inspect charts

Hits and misses are different questions. Overlaying them on one axis makes a quiet miss look like a dip in a hit line. Separate charts, one point per root thread, keep the counts readable.

## Roster is advice, not a scoreboard

Tiers are hit rate against contact count. Remove and optimize are for skills that fire and then do nothing, or that mix HIT and MISS. Keep is the default when the numbers do not argue. The list is only Skills-lane nodes with contacts. Playbooks and principles stay on the graph.

## What is left out

Token spend stays `Unknown` until a later hook reports it. Parent model is not in the transcript. Child-thread Reads are judged on the board. The courier stays on the root thread, which is the prompt you typed.

Ambient is also narrower than the window suggests. A Read counts as ambient when the session has no `/nax-mode` slash. That was true for the fork this crate started on. It is the wrong default for a plugin that uses another mode skill. The graph still shows those Reads. The roster treats them as unused context.
