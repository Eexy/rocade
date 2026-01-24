-- Add migration script here
create table if not exists genres (
 id integer primary key  autoincrement  not null,
    name TEXT not null
);

create table if not exists games_genres(
    id integer primary key autoincrement not null,
    game_id integer not null,
    genre_id integer not null,

    foreign key (game_id) references games(id),
    foreign key (genre_id) references genres(id)
);