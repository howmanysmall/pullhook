import { access, constants, stat } from "node:fs/promises";
import { textDecoder } from "@constants/reused-constants";

import type { PathLike } from "node:fs";

const BUFFER_EXISTS = typeof Buffer !== "undefined";

export function fromPathLike(pathLike: PathLike): string {
	if (typeof pathLike === "string") return pathLike;
	if (pathLike instanceof URL) return pathLike.pathname;
	if (BUFFER_EXISTS && Buffer.isBuffer(pathLike)) return pathLike.toString();

	if (pathLike instanceof ArrayBuffer || ArrayBuffer.isView(pathLike)) {
		const bytes =
			pathLike instanceof ArrayBuffer
				? new Uint8Array(pathLike)
				: new Uint8Array(pathLike.buffer, pathLike.byteOffset, pathLike.byteLength);

		return textDecoder.decode(bytes);
	}

	const error = new TypeError(`Unsupported path type: ${Object.prototype.toString.call(pathLike)}`);
	Error.captureStackTrace(error, fromPathLike);
	throw error;
}

export async function fileExistsAsync(pathLike: PathLike): Promise<boolean> {
	try {
		const filePath = fromPathLike(pathLike);
		await access(filePath, constants.F_OK);
		const fileStat = await stat(filePath);
		return fileStat.isFile() || fileStat.isFIFO();
	} catch {
		return false;
	}
}
