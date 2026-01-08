<template>
    <div v-if="currentGame" class="flex text-foreground justify-center">
        <div class="w-5/6 mx-auto">
            <div class="relative rounded-xl overflow-hidden">
                <div class="aspect-21/9 relative">
                    <img v-if="artworkUrl" :src="artworkUrl" class="w-full h-full object-cover" />
                    <div class="absolute inset-0 bg-linear-to-t from-background via-transparent to-transparent">
                    </div>
                </div>
                <div class="py-8">
                    <h1 class="text-3xl font-semibold">{{ currentGame.name }}</h1>
                    <p>{{ currentGame.summary }}</p>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
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


const artworkUrl = computed(() => {
    if (!currentGame.value) return null

    if (!currentGame.value.artworks) return null

    if (!currentGame.value.artworks[0]) return null
    return `https://images.igdb.com/igdb/image/upload/t_1080p/${currentGame.value.artworks[0]}.jpg`
})

</script>

<style scoped></style>
