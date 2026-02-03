create table temp_studios
(
    id      integer primary key autoincrement,
    igdb_id integer unique not null,
    name    text           not null
);

insert into temp_studios(igdb_id, name)
select igdb_id, name
from studios;


create table temp_games_studios
(
    id        integer primary key autoincrement,
    game_id   integer not null,
    studio_id integer not null,

    foreign key (game_id) references games (id),
    foreign key (studio_id) references temp_studios (id)
);

insert into temp_games_studios(game_id, studio_id)
select games.id as game_id, temp_studios.id as studio_id
    from games
        left join games_studios on games_studios.game_id = games.id
    left join studios on games_studios.studio_id = studios.id
    left join temp_studios on studios.name = temp_studios.name;

drop table games_studios;
drop table studios;

alter table temp_studios rename to studios;
alter table temp_games_studios rename to games_studios;