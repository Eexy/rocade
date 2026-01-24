-- Add migration script here
alter table games drop column release_date;

alter table games add column release_date integer;