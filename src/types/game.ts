export type Game = {
    appid: number,
    name: string,
    img_icon_url?: string,
    img_logo_url?: string,
}

export type GameInfo = {
    id: number,
    name: String,
    cover: GameImage,
    artworks: GameImage[],
    genres: GameGenre[],
    storyline?: string,
    summary?: string,

}

export type GameGenre = {
    name: string
}

export type GameImage = {
    image_id: string
}
