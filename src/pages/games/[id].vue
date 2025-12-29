<template>
    <div v-if="currentGame" class="flex flex-col gap-4 p-2">
        <div>
            <img v-if="coverUrl" :src="coverUrl" class="rounded-sm" />
        </div>
        <h1 class="text-3xl font-semibold">{{ currentGame.name }}</h1>
        <div class="flex gap-1">
            <Badge v-for="genre in currentGame.genres" :key="genre.name">{{ genre.name }}</Badge>
        </div>
        <p>{{ currentGame.summary }}</p>
    </div>
</template>

<script setup lang="ts">
import Badge from '@/components/ui/badge/Badge.vue';
import { useGameStore } from '@/stores/game.store';
import { GameInfo } from '@/types/game';
import { invoke } from '@tauri-apps/api/core';
import { storeToRefs } from 'pinia';
import { computed, watch, ref } from 'vue';
import { useRoute } from 'vue-router';

const route = useRoute('/games/[id]');
const id = computed(() => Number(route.params.id))

const { games } = storeToRefs(useGameStore())
const currentGame = ref<GameInfo | null>(null)

const coverUrl = computed(() => {
    if (!currentGame.value) return null

    if (!currentGame.value.covers[0]) return null
    return `https://images.igdb.com/igdb/image/upload/t_cover_big/${currentGame.value.covers[0].image_id}.jpg`
})

watch(() => id, async () => {
    const game = games.value.find(g => g.appid === id.value)
    if (!game) return

    try {
        const res: GameInfo = await invoke('get_game', { name: game.name })
        currentGame.value = res
    } catch (e) {
        console.error(e)
    }
}, { immediate: true, deep: true })


</script>

<style scoped></style>
