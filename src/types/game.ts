export type GameInfo = {
    id: number,
    name: String,
    cover?: string,
    artworks?: string[],
    genres: GameGenre[],
    storyline?: string,
    summary?: string,

}

export type GameGenre = {
    name: string
}
