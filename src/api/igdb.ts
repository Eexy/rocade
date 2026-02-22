import { IgdbImgSize } from "@/types/igdb";
import { GameImage } from "@/types/game";
import { convertFileSrc } from '@tauri-apps/api/core';

export function getIgdbImageUrl(id: string, size: IgdbImgSize): string {
    return `https://images.igdb.com/igdb/image/upload/${size}/${id}.jpg`
}

/**
 * Gets the URL for a game image, preferring local cached version.
 *
 * @param image - GameImage object or legacy string image ID
 * @param size - IGDB image size (used only for CDN fallback)
 * @returns Image URL (local file or IGDB CDN), or undefined if no image
 */
export function getGameImageUrl(
    image: GameImage | string | undefined,
    size: IgdbImgSize
): string | undefined {
    if (!image) return undefined;

    // Handle legacy string format (backwards compatibility)
    if (typeof image === 'string') {
        return getIgdbImageUrl(image, size);
    }

    // Prefer local path
    if (image.local_path) {
        return convertFileSrc(image.local_path);
    }

    // Fallback to IGDB CDN
    return getIgdbImageUrl(image.id, size);
}
