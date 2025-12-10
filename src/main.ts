import { createApp } from "vue";
import "./index.css"
import App from "./App.vue";
import { createPinia } from "pinia"
import { createRouter, createWebHistory } from "vue-router";
import Game from "./views/games/Game.vue";

const pinia = createPinia()
const router = createRouter({
    history: createWebHistory(), routes: [
        { path: '/', redirect: '/games' },
        { path: '/games', component: Game }
    ]
});

createApp(App).use(pinia).use(router).mount("#app");
