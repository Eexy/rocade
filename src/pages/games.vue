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

</script>

<style scoped></style>
