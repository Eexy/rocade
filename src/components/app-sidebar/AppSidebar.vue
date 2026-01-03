<template>
    <Sidebar>
        <SidebarHeader>
            <Input v-model="search"></Input>
        </SidebarHeader>
        <SidebarContent>
            <ul>
                <RouterLink v-for="game in filteredGames" :to="`/games/${game.name}`">
                    <GameSidebarItem :game="game"></GameSidebarItem>
                </RouterLink>
            </ul>
        </SidebarContent>
    </Sidebar>
</template>

<script setup lang="ts">
import { useGameStore } from '@/stores/game.store';
import { storeToRefs } from 'pinia';
import { Sidebar, SidebarContent, SidebarHeader } from "@/components/ui/sidebar"
import { Input } from "@/components/ui/input"
import GameSidebarItem from "@/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue";

const { filteredGames, search } = storeToRefs(useGameStore())

</script>

<style scoped></style>
