<template>
    <div>
        <p>hello world {{ id }}</p>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';


const route = useRoute('/games/[id]')
const id = computed(() => {
    return route.params.id ?? ''
})


</script>

<style scoped></style>
