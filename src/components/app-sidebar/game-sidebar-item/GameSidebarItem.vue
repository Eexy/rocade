<template>
    <li class="flex gap-2 items-center">
        <img :src="coverUrl" class="size-9" />
        {{ game.name }}
    </li>
</template>

<script setup lang="ts">
import { getGameImageUrl } from '@/api/igdb';
import { GameInfo } from '@/types/game';
import { computed } from 'vue';

type Props = {
    game: GameInfo
}

const props = defineProps<Props>()


const coverUrl = computed(() => {
    return getGameImageUrl(props.game.cover, 't_cover_small')
})

</script>

<style scoped></style>
