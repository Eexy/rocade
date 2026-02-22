export interface GameImage {
    id: string
    local_path?: string
}

export type GameInfo = {
    id: number,
    name: string,
    is_installed: boolean,
    cover?: GameImage,
    artworks?: GameImage[],
    genres?: string[],
    storyline?: string,
    summary?: string,
    store_id?: string
    release_date?: number
    developers?: string[]
}

