<template>
    <Sidebar>
        <SidebarHeader>
            <InputGroup>
                <InputGroupInput placeholder="Search games" v-model="search"></InputGroupInput>
                <InputGroupAddon>
                    <SearchIcon></SearchIcon>
                </InputGroupAddon>
            </InputGroup>
        </SidebarHeader>
        <SidebarContent>
            <ul>
                <RouterLink v-for="game in filteredGames" :key="game.id" :to="`/games/${game.id}`">
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
import { InputGroup, InputGroupAddon, InputGroupInput } from '../ui/input-group';
import { SearchIcon } from 'lucide-vue-next';
import GameSidebarItem from "@/components/app-sidebar/game-sidebar-item/GameSidebarItem.vue";

const { filteredGames, search } = storeToRefs(useGameStore())

</script>

<style scoped></style>
