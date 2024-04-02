-- Add migration script here

CREATE TABLE IF NOT EXISTS shifts
(
    id          INTEGER PRIMARY KEY NOT NULL,
    time_in     INTEGER             NOT NULL DEFAULT(unixepoch()),
    time_out    INTEGER,
    time_diff   INTEGER
);
    -- active      BOOLEAN             NOT NULL DEFAULT 0


CREATE TABLE IF NOT EXISTS logs
(
    id          INTEGER PRIMARY KEY NOT NULL,
    shift_id    INTEGER            NOT NULL,
    event       VARCHAR(140)       NOT NULL,
    created_at  TIMESTAMP          NOT NULL DEFAULT(unixepoch())
);

