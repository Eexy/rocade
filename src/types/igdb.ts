type ImagePrefix<T extends string> = `t_${T}`

export type ImageSize = 'cover_small' | 'screenshot_med' | 'cover_big' | 'logo_med' | 'screenshot_big' | 'thumb' | 'micro' | '720p' | '1080p'

export type IgdbImage = ImagePrefix<ImageSize>


