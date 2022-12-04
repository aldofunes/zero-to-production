CREATE TABLE users (
  user_id uuid PRIMARY KEY,
  username text NOT NULL UNIQUE,
  PASSWORD text NOT NULL
);