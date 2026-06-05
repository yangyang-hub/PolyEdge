-- Add wallet_address to reward_account_state for displaying the Polymarket wallet
-- address configured in the worker service.
ALTER TABLE reward_account_state ADD COLUMN IF NOT EXISTS wallet_address TEXT;
