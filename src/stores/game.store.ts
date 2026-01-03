import { Game, GameInfo } from "@/types/game";
import { defineStore } from "pinia";
import { computed, ref } from "vue";

export const useGameStore = defineStore('game', () => {
    const games = ref<GameInfo[]>([]);
    const search = ref("");

    const filteredGames = computed(() => {
        if (!search.value.length) return games.value

        return games.value.filter((game) => game.name.toLowerCase().includes(search.value.toLowerCase()))
    })

    return { games, filteredGames, search }
})
