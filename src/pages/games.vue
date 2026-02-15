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
import AppSidebar from "@/components/app-sidebar/AppSidebar.vue"
import { useGameStore } from "@/stores/game.store";
import { onMounted } from "vue";

const gameStore = useGameStore()
onMounted(() => gameStore.init())

</script>

<style scoped></style>
