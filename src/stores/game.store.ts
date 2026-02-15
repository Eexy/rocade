import { getGames, refreshGames } from "@/commands/game.command";
import { GameInfo } from "@/types/game";
import { defineStore } from "pinia";
import { onMounted, ref, watch } from "vue";

export const useGameStore = defineStore('game', () => {
    const games = ref<GameInfo[]>([]);
    const search = ref("");
    const filteredGames = ref<GameInfo[]>([])

    async function init() {

        let res: GameInfo[] = await getGames()

        if (!res.length) {
            await refreshGames()
            res = await getGames();
        }

        games.value = res
    }

    watch([games, search], async () => {
        if (games.value.length && !search.value.length) {
            filteredGames.value = games.value
            return
        }

        const res = await getGames({ name: search.value })
        filteredGames.value = res
    }, { immediate: true })


    return { games, init, filteredGames, search }
})
