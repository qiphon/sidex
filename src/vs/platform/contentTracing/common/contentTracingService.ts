/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { Disposable } from '../../../base/common/lifecycle.js';
import { invoke } from '@tauri-apps/api/core';

export interface IContentTracingService {
	startSession(sessionId: string): Promise<void>;
	addEvent(sessionId: string, name: string, category: string, phase: string, args?: any): Promise<void>;
	addCompleteEvent(sessionId: string, name: string, category: string, durationUs: number, args?: any): Promise<void>;
	stopSession(sessionId: string): Promise<void>;
	exportTrace(sessionId: string): Promise<string>;
	getState(sessionId: string): Promise<ITraceState>;
	clearSession(sessionId: string): Promise<void>;
}

export interface ITraceState {
	sessionId: string;
	isRecording: boolean;
	eventCount: number;
	durationMs: number;
}

export class TauriContentTracingService extends Disposable implements IContentTracingService {
	constructor() {
		super();
	}

	async startSession(sessionId: string): Promise<void> {
		await invoke('tracing_start_session', { sessionId });
	}

	async addEvent(sessionId: string, name: string, category: string, phase: string, args?: any): Promise<void> {
		await invoke('tracing_add_event', { sessionId, name, category, phase, args });
	}

	async addCompleteEvent(sessionId: string, name: string, category: string, durationUs: number, args?: any): Promise<void> {
		await invoke('tracing_add_complete_event', { sessionId, name, category, durationUs, args });
	}

	async stopSession(sessionId: string): Promise<void> {
		await invoke('tracing_stop_session', { sessionId });
	}

	async exportTrace(sessionId: string): Promise<string> {
		return await invoke('tracing_export_trace', { sessionId });
	}

	async getState(sessionId: string): Promise<ITraceState> {
		return await invoke('tracing_get_state', { sessionId });
	}

	async clearSession(sessionId: string): Promise<void> {
		await invoke('tracing_clear_session', { sessionId });
	}
}
