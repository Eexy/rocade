-- Add migration script here
ALTER TABLE covers ADD COLUMN local_path TEXT;
ALTER TABLE artworks ADD COLUMN local_path TEXT;
