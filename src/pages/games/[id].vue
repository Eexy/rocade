<template>
    <div v-if="currentGame" class="flex text-foreground justify-center">
        <div class="w-5/6 mx-auto">
            <div class="relative rounded-xl overflow-hidden">
                <div class="aspect-21/9 relative">
                    <img v-if="artworkUrl" :src="artworkUrl" class="w-full h-full object-cover" />
                    <div class="absolute inset-0 bg-linear-to-t from-background via-transparent to-transparent">
                    </div>
                    <div class="absolute bottom-0 p-6 flex flex-col gap-5">
                        <div>
                            <h1 class="text-5xl">{{ currentGame.name }}</h1>
                        </div>
                        <Button class="self-start py-6">
                            <span class="flex gap-4 px-2">
                                <PlayIcon class="size-5" fill="white" />
                                <span>Play</span>
                            </span>
                        </Button>
                    </div>
                </div>
                <div class="py-8">
                    <Card class="gap-3">
                        <CardHeader>
                            <CardTitle class="text-2xl">About</CardTitle>
                        </CardHeader>
                        <CardContent>
                            {{ currentGame.summary }}
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import Button from '@/components/ui/button/Button.vue';
import Card from '@/components/ui/card/Card.vue';
import CardContent from '@/components/ui/card/CardContent.vue';
import { PlayIcon } from 'lucide-vue-next';
import { useGameStore } from '@/stores/game.store';
import { storeToRefs } from 'pinia';
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import CardTitle from '@/components/ui/card/CardTitle.vue';
import CardHeader from '@/components/ui/card/CardHeader.vue';

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
