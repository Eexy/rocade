import { GameInfo } from "@/types/game";
import { invoke } from "@tauri-apps/api/core";

export async function getGames(): Promise<GameInfo[]> {
    return await invoke("get_games")
}

export async function getGameById(gameId: number): Promise<GameInfo> {
    return await invoke("get_game", { gameId })
}

export async function installGame(gameId: number): Promise<void> {
    return await invoke("install_game", { gameId })
}

export async function uninstallGame(gameId: number): Promise<void> {
    return await invoke("uninstall_game", { gameId })
}

export async function refreshGames(): Promise<void> {
    return await invoke('refresh_games')
}
