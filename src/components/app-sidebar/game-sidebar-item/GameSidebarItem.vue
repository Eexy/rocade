<template>
    <li class="flex gap-2 items-center">
        <img :src="coverUrl" class="size-9" />
        {{ game.name }}
    </li>
</template>

<script setup lang="ts">
import { GameInfo } from '@/types/game';
import { computed } from 'vue';

type Props = {
    game: GameInfo
}

const props = defineProps<Props>()


const coverUrl = computed(() => {
    return `https://images.igdb.com/igdb/image/upload/t_cover_big/${props.game.cover.image_id}.jpg`
})

</script>

<style scoped></style>
