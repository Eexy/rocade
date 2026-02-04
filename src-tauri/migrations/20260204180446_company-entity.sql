-- Add migration script here
alter table studios rename to companies;

alter table games_studios rename to developed_by;
