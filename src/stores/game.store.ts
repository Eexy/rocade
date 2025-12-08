import { Game } from "@/types/game";
import { defineStore } from "pinia";
import { ref } from "vue";

export const useGameStore = defineStore('game', () => {
    const games = ref<Game[]>([]);

    return { games }
})
