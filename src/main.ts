import { createApp } from "vue";
import "./index.css";
import App from "./App.vue";
import { createPinia } from "pinia";
import { createRouter, createWebHistory } from "vue-router";
import { routes as generatedRoutes } from "vue-router/auto-routes";

const pinia = createPinia();
const router = createRouter({
    history: createWebHistory(),
    routes: [
        { path: "/", redirect: "/games" },
        ...generatedRoutes,
    ],
});

createApp(App).use(pinia).use(router).mount("#app");
