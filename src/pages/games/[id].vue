<template>
    <div v-if="game" class="flex text-foreground justify-center">
        <div class="w-5/6 mx-auto">
            <div class="relative rounded-xl overflow-hidden">
                <div class="aspect-21/9 relative">
                    <img v-if="artworkUrl" :src="artworkUrl" class="w-full h-full object-cover" />
                    <div class="absolute inset-0 bg-linear-to-t from-background via-transparent to-transparent">
                    </div>
                    <div class="absolute bottom-0 p-6 flex flex-col gap-5">
                        <div>
                            <h1 class="text-5xl">{{ game.name }}</h1>
                        </div>
                        <Button v-if="game.is_installed" class="self-start py-6">
                            <span class="flex gap-4 px-2">
                                <PlayIcon class="size-5" fill="white" />
                                <span>Play</span>
                            </span>
                        </Button>
                        <Button @click="onInstallClick" v-else class="self-start py-6">
                            <span class="flex gap-4 px-2">
                                <span>Download</span>
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
                            {{ game.summary }}
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
import { computed, ref, watchEffect } from 'vue';
import { useRoute } from 'vue-router';
import CardTitle from '@/components/ui/card/CardTitle.vue';
import CardHeader from '@/components/ui/card/CardHeader.vue';
import { invoke } from '@tauri-apps/api/core';
import { GameInfo } from '@/types/game';

const route = useRoute('/games/[id]');
const id = computed(() => Number(route.params.id))

const game = ref<GameInfo | null>(null)


watchEffect(async () => {
    game.value = await invoke('get_game', { gameId: id.value });
})


const artworkUrl = computed(() => {
    if (!game.value) return null

    if (!game.value.artworks) return null

    if (!game.value.artworks[0]) return null
    return `https://images.igdb.com/igdb/image/upload/t_1080p/${game.value.artworks[0]}.jpg`
})

async function onInstallClick() {
    if (!game.value) return

    if (!game.value.store_id) return
    const res = await invoke("install_game", { gameId: game.value.store_id })
    console.log(res)
}

</script>

<style scoped></style>
