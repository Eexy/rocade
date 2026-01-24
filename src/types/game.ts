export type GameInfo = {
    id: number,
    name: String,
    is_installed: boolean,
    cover?: string,
    artworks?: string[],
    genres: GameGenre[],
    storyline?: string,
    summary?: string,
    store_id?: string
    release_date?: number
}

export type GameGenre = {
    name: string
}
