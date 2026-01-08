<template>
    <div v-if="currentGame" class="flex flex-col gap-4 p-2">
        <div>
            <img v-if="artworkUrl" :src="artworkUrl" class="rounded-sm" />
        </div>
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
import { storeToRefs } from 'pinia';
import { computed } from 'vue';
import { useRoute } from 'vue-router';

const route = useRoute('/games/[id]');
const id = computed(() => Number(route.params.id))

const { games } = storeToRefs(useGameStore())

const currentGame = computed(() => {
    return games.value.find(game => game.id === id.value)
})

const coverUrl = computed(() => {
    if (!currentGame.value) return null

    if (!currentGame.value.cover) return null

    return `https://images.igdb.com/igdb/image/upload/t_cover_big/${currentGame.value.cover}.jpg`
})


const artworkUrl = computed(() => {
    if (!currentGame.value) return null

    if (!currentGame.value.artworks) return null

    if (!currentGame.value.artworks[0]) return null
    return `https://images.igdb.com/igdb/image/upload/t_cover_big/${currentGame.value.artworks[0]}.jpg`
})




</script>

<style scoped></style>
