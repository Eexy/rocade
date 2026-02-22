<template>
    <div v-if="game" class="flex text-foreground justify-center">
        <div class="w-5/6 mx-auto">
            <div class="relative rounded-xl overflow-hidden">
                <div class="relative bg-muted h-[400px] md:h-[500px] lg:h-[600px]">
                    <img v-if="artworkUrl" :src="artworkUrl" class="absolute inset-0 w-full h-full object-cover" />
                    <div
                        class="absolute inset-0 bg-linear-to-t from-background via-transparent to-transparent pointer-events-none">
                    </div>
                    <div class="absolute bottom-0 p-6 flex flex-col gap-5">
                        <div>
                            <h1 class="text-5xl">{{ game.name }}</h1>
                        </div>
                        <div class="text-muted-foreground flex items-center gap-2">
                            <template v-if="game.developers">
                                <div class="flex items-center gap-2">
                                    <span v-for="studio in game.developers" :key="studio"
                                        class="rounded bg-card px-1 capitalize">{{
                                            studio }}</span>
                                    <div>
                                        •
                                    </div>
                                </div>
                            </template>
                            <div v-if="releaseDate">{{ releaseDate }}</div>
                            <template v-if="game.genres">
                                <div>
                                    •
                                </div>
                                <div class="flex items-center gap-2">
                                    <span v-for="genre in game.genres" :key="genre"
                                        class="rounded bg-card px-1 capitalize">{{
                                            genre }}</span>
                                </div>
                            </template>
                        </div>
                        <template v-if="game.is_installed">
                            <div class="flex gap-3">
                                <Button class="self-start py-6">
                                    <span class="flex gap-4 px-2">
                                        <PlayIcon class="size-5" fill="white" />
                                        <span>Play</span>
                                    </span>
                                </Button>
                                <Button @click="onUninstallClick" variant="destructive" class="self-start py-6">
                                    <span class="flex gap-4 px-2">
                                        <XIcon class="size-5" fill="white" />
                                        <span>Uninstall</span>
                                    </span>
                                </Button>
                            </div>
                        </template>
                        <template v-else>
                            <Button @click="onDownloadClick" class="self-start py-6">
                                <span class="flex gap-4 px-2">
                                    <span>Download</span>
                                </span>
                            </Button>
                        </template>
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
import { PlayIcon, XIcon } from 'lucide-vue-next';
import { computed, ref, watchEffect } from 'vue';
import { useRoute } from 'vue-router';
import CardTitle from '@/components/ui/card/CardTitle.vue';
import CardHeader from '@/components/ui/card/CardHeader.vue';
import { GameInfo } from '@/types/game';
import { format } from "date-fns"
import { getGameById, installGame, uninstallGame } from '@/commands/game.command';
import { getGameImageUrl } from '@/api/igdb';

const route = useRoute('/games/[id]');
const id = computed(() => Number(route.params.id))

const game = ref<GameInfo | null>(null)

const releaseDate = ref<string | null>(null)


watchEffect(async () => {
    game.value = await getGameById(id.value);

    if (game.value && game.value.release_date) {
        releaseDate.value = format(new Date(game.value.release_date * 1000), "MMMM dd, yyyy")
    }

})


const artworkUrl = computed(() => {
    if (!game.value) return null

    if (!game.value.artworks) return null

    if (!game.value.artworks[0]) return null

    return getGameImageUrl(game.value.artworks[0], 't_1080p')
})

async function onDownloadClick() {
    if (!game.value) return

    if (!game.value.store_id) return

    await installGame(game.value.id)
}

async function onUninstallClick() {
    if (!game.value) return

    if (!game.value.store_id) return

    await uninstallGame(game.value.id)
}



</script>

<style scoped></style>
