-- Explorer database, schema version 4 (`db::SCHEMA_VERSION`): the spine and its branches
-- (docs/plans/2026-10-23-explorer-spine.md). u64 values (digests, hashes, build ids) are stored
-- bit-cast to i64.
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,           -- schema, flavor ('stub'|'emerald'|'silice'), build_engine,
                                    -- build_content, key_version, act
  value TEXT NOT NULL
) STRICT;

-- The last spine run (one row).
CREATE TABLE IF NOT EXISTS spine (
  id            INTEGER PRIMARY KEY CHECK (id = 1),
  steps         INTEGER NOT NULL,             -- steps in the list
  done          INTEGER NOT NULL,             -- steps that ran
  frames        INTEGER NOT NULL,
  inputs        BLOB    NOT NULL,             -- runs::to_blob from a new game
  digest        INTEGER NOT NULL,             -- state_digest of the last position
  error         TEXT,                         -- why it stopped early (NULL: complete)
  wall_ms       INTEGER NOT NULL,
  build_engine  INTEGER NOT NULL,
  build_content INTEGER NOT NULL
) STRICT;

-- The spine's positions (after each step; 0 = the settled new game).
CREATE TABLE IF NOT EXISTS position (
  idx     INTEGER PRIMARY KEY,
  label   TEXT,
  segment TEXT,
  frame   INTEGER NOT NULL,
  map     INTEGER NOT NULL,
  x       INTEGER NOT NULL,
  y       INTEGER NOT NULL,
  digest  INTEGER NOT NULL
) STRICT;

-- One row per branch (origin, action); a later run replaces it.
CREATE TABLE IF NOT EXISTS branch (
  id             INTEGER PRIMARY KEY,
  origin         INTEGER NOT NULL,            -- position idx
  action         TEXT    NOT NULL,            -- branch::Action RON
  kind           TEXT    NOT NULL,            -- TargetKind::name
  outcome        TEXT    NOT NULL,            -- Outcome::name
  explored       TEXT,                        -- exhausted | low_novelty | budget
  macros         TEXT    NOT NULL,            -- the exploration's macros, RON
  explore_frames INTEGER NOT NULL,
  recover_frames INTEGER NOT NULL,
  window_frames  INTEGER NOT NULL,
  new_maps       INTEGER NOT NULL,
  new_scripts    INTEGER NOT NULL,
  new_flags      INTEGER NOT NULL,
  new_vars       INTEGER NOT NULL,
  new_cells      INTEGER NOT NULL,
  points         INTEGER NOT NULL,
  first_found    INTEGER NOT NULL,            -- seen rows this branch found first
  message        TEXT    NOT NULL,
  run            INTEGER NOT NULL,
  UNIQUE (origin, action)
) STRICT;

-- Novelty archive: what branches found that the spine never touched (SeenKind).
CREATE TABLE IF NOT EXISTS seen (
  kind         INTEGER NOT NULL,
  id           INTEGER NOT NULL,
  first_branch INTEGER NOT NULL,              -- branch.id
  hits         INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (kind, id)
) STRICT, WITHOUT ROWID;

-- Cells stood on (the studio heatmap): by the spine, and by branches.
CREATE TABLE IF NOT EXISTS coverage (
  map    INTEGER NOT NULL,
  x      INTEGER NOT NULL,
  y      INTEGER NOT NULL,
  spine  INTEGER NOT NULL DEFAULT 0,
  hits   INTEGER NOT NULL DEFAULT 0,          -- branches that stood on it
  PRIMARY KEY (map, x, y)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS finding (
  id            INTEGER PRIMARY KEY,
  signature     INTEGER NOT NULL UNIQUE,
  kind          TEXT    NOT NULL,
  branch        INTEGER,                      -- branch.id; NULL = the spine
  origin        INTEGER,                      -- position idx the tail starts at; NULL = a new game
  macro         TEXT,
  inputs        BLOB    NOT NULL,             -- runs::to_blob from a new game to the finding (inclusive)
  tail_frames   INTEGER NOT NULL,             -- the last frames of `inputs` that are the tail
  total_frame   INTEGER NOT NULL,
  map           INTEGER NOT NULL,
  x             INTEGER NOT NULL,
  y             INTEGER NOT NULL,
  message       TEXT    NOT NULL,
  location      TEXT,
  script        INTEGER,
  log           TEXT    NOT NULL,
  build_engine  INTEGER NOT NULL,
  build_content INTEGER NOT NULL,
  status        TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open','stale')),
  hits          INTEGER NOT NULL DEFAULT 1,
  first_run     INTEGER NOT NULL,
  last_run      INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS run (                  -- timings: never in content_digest
  id       INTEGER PRIMARY KEY,
  started  TEXT    NOT NULL,
  command  TEXT    NOT NULL,                      -- spine | branch
  config   TEXT    NOT NULL,                      -- RON of Config
  threads  INTEGER NOT NULL,
  branches INTEGER NOT NULL,
  frames   INTEGER NOT NULL,
  wall_ms  INTEGER NOT NULL
) STRICT;
