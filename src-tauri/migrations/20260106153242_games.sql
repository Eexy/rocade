-- Add migration script here
create table if not exists games(
    id integer primary key autoincrement,
    name text not null,
    summary text
)