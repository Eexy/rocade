import { createApp } from "vue";
import "./index.css"
import App from "./App.vue";
import { createPinia } from "pinia"
import { createRouter, createWebHistory } from "vue-router";
import Games from "./views/games/Game.vue";

const pinia = createPinia()
const router = createRouter({
    history: createWebHistory(), routes: [
        { path: '/', redirect: '/games' },
        { path: '/games', component: Games }
    ]
});

createApp(App).use(pinia).use(router).mount("#app");
