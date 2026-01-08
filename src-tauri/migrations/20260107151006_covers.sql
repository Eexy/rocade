-- Add migration script here
create table if not exists covers (
    id integer primary key autoincrement,
    game_id integer not null,
    cover_id text not null,

    foreign key (game_id) references games(id)
);
