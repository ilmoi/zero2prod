-- Add migration script here
CREATE TABLE subscription_tokens(
    id uuid NOT NULL,
    PRIMARY KEY (id),
    sub uuid NOT NULL,
    FOREIGN KEY(sub)
        REFERENCES subscriptions (id),
    sub_token TEXT NOT NULL
)