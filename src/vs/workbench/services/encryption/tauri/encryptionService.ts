/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { IEncryptionService, KnownStorageProvider } from '../../../../platform/encryption/common/encryptionService.js';
import { InstantiationType, registerSingleton } from '../../../../platform/instantiation/common/extensions.js';
import { invoke } from '@tauri-apps/api/core';

/**
 * Tauri 环境的加密服务实现
 * 使用 Web Crypto API 进行 AES-GCM 加密，密钥通过 Rust 侧的 OS keyring 保护
 */
export class TauriEncryptionService implements IEncryptionService {
	declare readonly _serviceBrand: undefined;

	private _key: CryptoKey | null = null;
	private _encryptionAvailable: boolean = false;

	constructor() {
		this._initializeKey();
	}

	private async _initializeKey(): Promise<void> {
		try {
			// 尝试从 Rust 侧获取加密密钥（通过 OS keyring 保护）
			const keyMaterial = await invoke<string>('secret_get', { key: '__encryption_master_key__' });

			if (keyMaterial) {
				// 使用存储的密钥
				const keyData = Uint8Array.from(atob(keyMaterial), c => c.charCodeAt(0));
				this._key = await crypto.subtle.importKey(
					'raw',
					keyData,
					'AES-GCM',
					false,
					['encrypt', 'decrypt']
				);
				this._encryptionAvailable = true;
			} else {
				// 生成新密钥并存储
				const newKey = await crypto.subtle.generateKey(
					{ name: 'AES-GCM', length: 256 },
					true,
					['encrypt', 'decrypt']
				);

				const exportedKey = await crypto.subtle.exportKey('raw', newKey);
				const keyBase64 = btoa(String.fromCharCode(...new Uint8Array(exportedKey)));

				await invoke('secret_set', { key: '__encryption_master_key__', value: keyBase64 });

				this._key = newKey as CryptoKey;
				this._encryptionAvailable = true;
			}
		} catch (error) {
			console.warn('[EncryptionService] Failed to initialize encryption key:', error);
			this._encryptionAvailable = false;
		}
	}

	async encrypt(value: string): Promise<string> {
		if (!this._key || !this._encryptionAvailable) {
			// 如果加密不可用，返回明文（降级）
			return value;
		}

		try {
			const iv = crypto.getRandomValues(new Uint8Array(12));
			const encoded = new TextEncoder().encode(value);

			const encrypted = await crypto.subtle.encrypt(
				{ name: 'AES-GCM', iv },
				this._key,
				encoded
			);

			// 组合 IV 和密文
			const result = new Uint8Array(iv.length + encrypted.byteLength);
			result.set(iv);
			result.set(new Uint8Array(encrypted), iv.length);

			// 转换为 base64
			return btoa(String.fromCharCode(...result));
		} catch (error) {
			console.warn('[EncryptionService] Encryption failed:', error);
			return value; // 降级为明文
		}
	}

	async decrypt(value: string): Promise<string> {
		if (!this._key || !this._encryptionAvailable) {
			// 如果加密不可用，返回原文（降级）
			return value;
		}

		try {
			// 从 base64 解码
			const data = Uint8Array.from(atob(value), c => c.charCodeAt(0));

			// 提取 IV 和密文
			const iv = data.slice(0, 12);
			const ciphertext = data.slice(12);

			const decrypted = await crypto.subtle.decrypt(
				{ name: 'AES-GCM', iv },
				this._key,
				ciphertext
			);

			return new TextDecoder().decode(decrypted);
		} catch (error) {
			console.warn('[EncryptionService] Decryption failed:', error);
			return value; // 降级为原文
		}
	}

	async isEncryptionAvailable(): Promise<boolean> {
		return this._encryptionAvailable;
	}

	async getKeyStorageProvider(): Promise<KnownStorageProvider> {
		if (this._encryptionAvailable) {
			return KnownStorageProvider.systemKeyring;
		}
		return KnownStorageProvider.basicText;
	}

	async setUsePlainTextEncryption(): Promise<void> {
		this._encryptionAvailable = false;
	}
}

registerSingleton(IEncryptionService, TauriEncryptionService, InstantiationType.Delayed);
