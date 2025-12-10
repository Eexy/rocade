<template>
    <Sidebar>
        <SidebarHeader>
            <Input v-model="search"></Input>
        </SidebarHeader>
        <SidebarContent>
            <ul>
                <RouterLink v-for="game in filteredGames" :key="game.appid" :to="`/games/${game.appid}`">
                    <li class="flex gap-2 items-center">
                        <img :src="`http://media.steampowered.com/steamcommunity/public/images/apps/${game.appid}/${game.img_icon_url}.jpg`"
                            class="size-9" />
                        {{ game.name }}
                    </li>
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

const { filteredGames, search } = storeToRefs(useGameStore())

</script>

<style scoped></style>
