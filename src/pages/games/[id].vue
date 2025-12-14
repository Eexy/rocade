<template>
    <div>
        <p>hello world {{ id }} </p>
    </div>
</template>

<script setup lang="ts">
import { useGameStore } from '@/stores/game.store';
import { invoke } from '@tauri-apps/api/core';
import { storeToRefs } from 'pinia';
import { computed, watch } from 'vue';
import { useRoute } from 'vue-router';

const route = useRoute('/games/[id]');
const id = computed(() => route.params.id)

const { games } = storeToRefs(useGameStore())

watch(() => id, async () => {
    const game = games.value.find(g => g.appid === Number(id.value))
    if (!game) return
    const res = await invoke('get_game', { name: game.name })
    console.log(res)
}, { immediate: true })


</script>

<style scoped></style>
