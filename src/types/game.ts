export type GameInfo = {
    id: number,
    name: String,
    is_installed: boolean,
    cover?: string,
    artworks?: string[],
    genres?: string[],
    storyline?: string,
    summary?: string,
    store_id?: string
    release_date?: number
}

