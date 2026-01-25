-- Add migration script here
create table if not exists studios (
    id integer primary key autoincrement  not null,
    igdb_id integer not null unique,
    name TEXT
);

create table if not exists games_studios(
    id integer primary key autoincrement not null,
    game_id integer not null,
    studio_id integer not null,

    foreign key (game_id) references games(id),
    foreign key (studio_id) references studios(id)
)