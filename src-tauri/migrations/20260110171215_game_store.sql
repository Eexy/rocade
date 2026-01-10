-- Add migration script here
create table if not exists games_store (
    id integer primary key autoincrement not null,
    game_id integer not null,
    store_id text not null,

    foreign key (game_id) references games(id)
)