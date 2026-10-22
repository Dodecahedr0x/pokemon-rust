-- Explorer database, schema version 1 (`db::SCHEMA_VERSION`).
-- u64 values (keys, digests, hashes, build ids) are stored bit-cast to i64.
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,           -- schema, flavor ('stub'|'emerald'), build_engine, build_content, seed, generation
  value TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS node (
  id             INTEGER PRIMARY KEY,
  parent         INTEGER REFERENCES node(id),   -- NULL = root (new game)
  depth          INTEGER NOT NULL,
  macro          TEXT    NOT NULL,              -- RON of Macro
  inputs         TEXT    NOT NULL,              -- runs text from the parent's end state
  frames         INTEGER NOT NULL,
  total_frames   INTEGER NOT NULL,
  end_frame      INTEGER NOT NULL,
  key            INTEGER NOT NULL,
  region         INTEGER NOT NULL,
  digest         INTEGER NOT NULL,              -- state_digest of the end snapshot
  world_hash     INTEGER NOT NULL,
  map            INTEGER NOT NULL,
  x              INTEGER NOT NULL,
  y              INTEGER NOT NULL,
  progress       INTEGER NOT NULL,
  f_novelty      INTEGER NOT NULL,
  f_progress     INTEGER NOT NULL,
  f_bug          INTEGER NOT NULL,
  f_balance      INTEGER NOT NULL,
  f_total        INTEGER NOT NULL,
  generation     INTEGER NOT NULL,
  expanded       INTEGER NOT NULL DEFAULT 0,
  status         TEXT    NOT NULL CHECK (status IN ('valid','stale','merged','broken')),
  merged_into    INTEGER REFERENCES node(id),
  build_engine   INTEGER NOT NULL,
  build_content  INTEGER NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS node_parent ON node(parent);
CREATE INDEX IF NOT EXISTS node_key    ON node(key);
CREATE INDEX IF NOT EXISTS node_map    ON node(map);
-- One live node per abstract key ('merged' rows keep their key for history).
CREATE UNIQUE INDEX IF NOT EXISTS node_live_key ON node(key) WHERE status IN ('valid','stale');

CREATE TABLE IF NOT EXISTS node_touch (           -- incremental re-simulation (v1)
  node INTEGER NOT NULL REFERENCES node(id),
  kind INTEGER NOT NULL,                          -- 0 map, 1 script
  id   INTEGER NOT NULL,
  PRIMARY KEY (node, kind, id)
) STRICT, WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS touch_by_id ON node_touch(kind, id);

CREATE TABLE IF NOT EXISTS seen (                 -- novelty archive
  kind       INTEGER NOT NULL,                    -- SeenKind
  id         INTEGER NOT NULL,                    -- flag id, (var << 16 | value), map id, script id
  first_node INTEGER NOT NULL REFERENCES node(id),
  hits       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (kind, id)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS coverage (             -- studio heatmap
  map        INTEGER NOT NULL,
  x          INTEGER NOT NULL,
  y          INTEGER NOT NULL,
  first_node INTEGER NOT NULL REFERENCES node(id),
  hits       INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (map, x, y)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS battle (               -- balance tables (v0: recorded, weight 0)
  id               INTEGER PRIMARY KEY,
  node             INTEGER NOT NULL REFERENCES node(id),
  kind             INTEGER NOT NULL,
  trainer          INTEGER,
  outcome          INTEGER,
  frames           INTEGER NOT NULL,
  player_level     INTEGER NOT NULL,
  opponent_level   INTEGER NOT NULL,
  hp_lost_permille INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS finding (
  id               INTEGER PRIMARY KEY,
  signature        INTEGER NOT NULL UNIQUE,
  kind             TEXT    NOT NULL,
  node             INTEGER NOT NULL REFERENCES node(id),  -- the tail starts at this node's end state
  macro            TEXT,
  tail             TEXT    NOT NULL,              -- runs text to the finding frame (inclusive)
  total_frame      INTEGER NOT NULL,              -- frames from a new game to the finding
  map              INTEGER NOT NULL,
  x                INTEGER NOT NULL,
  y                INTEGER NOT NULL,
  message          TEXT    NOT NULL,
  location         TEXT,
  script           INTEGER,
  log              TEXT    NOT NULL,
  build_engine     INTEGER NOT NULL,
  build_content    INTEGER NOT NULL,
  status           TEXT    NOT NULL DEFAULT 'open' CHECK (status IN ('open','stale','gone')),
  hits             INTEGER NOT NULL DEFAULT 1,
  first_generation INTEGER NOT NULL,
  last_generation  INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS snapshot (             -- disk snapshot cache (git-ignored db)
  node          INTEGER PRIMARY KEY REFERENCES node(id),
  build_engine  INTEGER NOT NULL,
  build_content INTEGER NOT NULL,
  bytes         BLOB    NOT NULL                  -- engine::checkpoint::encode
) STRICT;

CREATE TABLE IF NOT EXISTS run (                  -- timings: never in content_digest
  id          INTEGER PRIMARY KEY,
  started     TEXT    NOT NULL,
  config      TEXT    NOT NULL,                   -- RON of Config
  threads     INTEGER NOT NULL,
  generations INTEGER NOT NULL,
  nodes       INTEGER NOT NULL,
  frames      INTEGER NOT NULL,
  wall_ms     INTEGER NOT NULL
) STRICT;
