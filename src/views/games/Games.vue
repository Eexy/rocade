<template>
    <SidebarProvider>
        <AppSidebar></AppSidebar>
        <main class="">
            <SidebarTrigger></SidebarTrigger>
            <RouterView></RouterView>
        </main>
    </SidebarProvider>
</template>

<script setup lang="ts">
import { SidebarTrigger, SidebarProvider } from "@/components/ui/sidebar"
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core"
import { Game } from "@/types/game";
import { useGameStore } from "@/stores/game.store";
import { storeToRefs } from "pinia";
import AppSidebar from "@/components/app-sidebar/AppSidebar.vue"

const { games } = storeToRefs(useGameStore())

onMounted(async () => {
    const res: Game[] = await invoke("get_games")
    games.value = res
})

</script>

<style scoped></style>
