import Game from "./views/game/Game.vue"
import Games from "./Games.vue"

export const gameRouter =
    [{ path: '/', redirect: '/games' },
    {
        path: '/games', component: Games, children: [{
            path: ':id',
            component: Game
        }]
    }]
