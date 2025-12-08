<template>
    <SidebarProvider>
        <GameSidebar></GameSidebar>
        <main class="">
            <SidebarTrigger></SidebarTrigger>
            <Button @click="console.log('hello world')">Hello world!</Button>
        </main>
    </SidebarProvider>
</template>

<script setup lang="ts">
import { Button } from "@/components/ui/button"
import { SidebarTrigger, SidebarProvider } from "@/components/ui/sidebar"
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core"
import { Game } from "@/types/game";
import { useGameStore } from "@/stores/game.store";
import { storeToRefs } from "pinia";
import GameSidebar from "@/components/game-sidebar/GameSidebar.vue"

const { games } = storeToRefs(useGameStore())

onMounted(async () => {
    const res: Game[] = await invoke("get_games")
    games.value = res
})
</script>

<style scoped></style>
