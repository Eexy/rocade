import { createApp } from "vue";
import "./index.css"
import App from "./App.vue";
import { createPinia } from "pinia"
import { createRouter, createWebHistory } from "vue-router";
import Games from "./views/games/Games.vue";
import Game from "./views/games/views/game/Game.vue";

const pinia = createPinia()
const router = createRouter({
    history: createWebHistory(), routes: [
        { path: '/', redirect: '/games' },
        {
            path: '/games', component: Games, children: [{
                path: ':id',
                component: Game
            }]
        }
    ]
});

createApp(App).use(pinia).use(router).mount("#app");
