import { getGames } from "@/commands/game.command";
import { GameInfo } from "@/types/game";
import { defineStore } from "pinia";
import { ref, watch } from "vue";

export const useGameStore = defineStore('game', () => {
    const games = ref<GameInfo[]>([]);
    const search = ref("");
    const filteredGames = ref<GameInfo[]>([])

    watch([games, search], async () => {
        if (games.value.length && !search.value.length) {
            filteredGames.value = games.value
            return
        }

        const res = await getGames({ name: search.value })
        filteredGames.value = res
    }, { immediate: true })


    return { games, filteredGames, search }
})
