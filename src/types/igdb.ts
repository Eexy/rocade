type ImgPrefix<T extends string> = `t_${T}`

type ImgSize = 'cover_small' | 'screenshot_med' | 'cover_big' | 'logo_med' | 'screenshot_big' | 'thumb' | 'micro' | '720p' | '1080p'

export type IgdbImgSize = ImgPrefix<ImgSize>


