ALTER TABLE reward_low_competition_observations
    ADD COLUMN not_low_competition BOOLEAN NOT NULL DEFAULT false;
