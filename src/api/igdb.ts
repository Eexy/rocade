import { IgdbImgSize } from "@/types/igdb";

export function getIgdbImageUrl(id: string, size: IgdbImgSize): string {
    return `https://images.igdb.com/igdb/image/upload/${size}/${id}.jpg`
}
