-- Add migration script here
BEGIN;
    DROP TABLE subscription_tokens;
    CREATE TABLE subscription_tokens(
        sub_token TEXT NOT NULL,
        sub_id uuid NOT NULL
            REFERENCES subscriptions (id),
        PRIMARY KEY (sub_id)
    );
COMMIT;