<template>
    <Sidebar>
        <SidebarContent>
            <ul>
                <li v-for="game in games" :key="game.appid" class="flex gap-2 items-center">
                    <img :src="`http://media.steampowered.com/steamcommunity/public/images/apps/${game.appid}/${game.img_icon_url}.jpg`"
                        class="size-9" />
                    {{ game.name }}
                </li>
            </ul>
        </SidebarContent>
    </Sidebar>
</template>

<script setup lang="ts">
import { useGameStore } from '@/stores/game.store';
import { storeToRefs } from 'pinia';
import { Sidebar, SidebarContent } from "@/components/ui/sidebar"

const { games } = storeToRefs(useGameStore())

</script>

<style scoped></style>
