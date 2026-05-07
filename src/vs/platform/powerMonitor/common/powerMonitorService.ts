/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { Emitter, Event } from '../../../base/common/event.js';
import { Disposable } from '../../../base/common/lifecycle.js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface IPowerMonitorService {
	readonly onDidChangePowerState: Event<IPowerEvent>;
	getPowerStatus(): Promise<IPowerStatus>;
	startListening(): Promise<void>;
}

export interface IPowerStatus {
	onBatteryPower: boolean;
	acLineStatus?: string;
	batteryFlag?: number;
	batteryLifePercent?: number;
}

export interface IPowerEvent {
	type: 'suspend' | 'resume' | 'ac-changed' | 'shutdown' | 'lock-screen' | 'unlock-screen';
	timestamp: number;
	onBatteryPower?: boolean;
}

export class TauriPowerMonitorService extends Disposable implements IPowerMonitorService {
	private readonly _onDidChangePowerState = this._register(new Emitter<IPowerEvent>());
	readonly onDidChangePowerState: Event<IPowerEvent> = this._onDidChangePowerState.event;

	private _listening = false;

	constructor() {
		super();
		this._register(this._onDidChangePowerState);
	}

	async getPowerStatus(): Promise<IPowerStatus> {
		const result = await invoke<any>('power_monitor_get_power_status');
		return {
			onBatteryPower: result.onBatteryPower ?? result.acLineStatus === 'offline',
			acLineStatus: result.acLineStatus,
			batteryFlag: result.batteryFlag,
			batteryLifePercent: result.batteryLifePercent,
		};
	}

	async startListening(): Promise<void> {
		if (this._listening) {
			return;
		}

		await invoke('power_monitor_start_listening');
		this._listening = true;

		// 监听来自 Rust 的电源事件
		listen('power-monitor-event', (event) => {
			const payload = event.payload as any;
			this._onDidChangePowerState.fire({
				type: payload.type,
				timestamp: payload.timestamp,
				onBatteryPower: payload.type === 'acChanged' ? payload.onBatteryPower : undefined,
			});
		});
	}
}
