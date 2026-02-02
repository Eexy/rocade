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
import { GameInfo } from "@/types/game";
import { useGameStore } from "@/stores/game.store";
import { storeToRefs } from "pinia";
import AppSidebar from "@/components/app-sidebar/AppSidebar.vue"
import { getGames, refreshGames } from "@/commands/game.command";

const { games } = storeToRefs(useGameStore())

onMounted(async () => {
    let res: GameInfo[] = await getGames()

    if (!res.length) {
        await refreshGames()
        res = await getGames();
    }

    games.value = res
})

</script>

<style scoped></style>
