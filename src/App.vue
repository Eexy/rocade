<template>
    <main class="">
        <Button @click="console.log('hello world')">Hello world!</Button>
    </main>
</template>

<script setup lang="ts">
import { Button } from "@/components/ui/button"
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core"

onMounted(async () => {
    const res = await invoke("get_games")
    console.log(res)
})
</script>

<style scoped></style>
