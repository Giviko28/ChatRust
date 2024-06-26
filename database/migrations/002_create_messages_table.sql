CREATE TABLE IF NOT EXISTS messages (
    id SERIAL PRIMARY KEY,
    sender VARCHAR REFERENCES users(name),
    receiver VARCHAR REFERENCES users(name),
    message VARCHAR NOT NULL
);