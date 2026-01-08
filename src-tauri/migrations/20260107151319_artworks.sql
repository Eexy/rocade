-- Add migration script here
create table if not exists artworks (
    id integer primary key autoincrement,
    game_id integer not null,
    artwork_id text not null,

    foreign key (game_id) references games(id)
)
