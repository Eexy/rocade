export type Game = {
    appid: number,
    name: string,
    img_icon_url?: string,
    img_logo_url?: string,
}

export type GameInfo = {
    name: String,
    covers: GameCover[],
    genres: GameGenre[],
    storyline?: string,
    summary?: string,

}

export type GameGenre = {
    name: string
}

export type GameCover = {
    image_id: string
}
