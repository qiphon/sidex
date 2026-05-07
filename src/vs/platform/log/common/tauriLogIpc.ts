/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { URI } from '../../../base/common/uri.js';
import { Event, Emitter } from '../../../base/common/event.js';
import { Disposable } from '../../../base/common/lifecycle.js';
import {
	ILogger,
	ILoggerOptions,
	ILoggerResource,
	ILoggerService,
	LogLevel
} from './log.js';
import { invoke } from '@tauri-apps/api/core';

/**
 * Tauri 日志桥接服务
 * 将 TypeScript 侧的日志请求转发到 Rust 侧的 LoggerStore
 */
export class TauriLoggerChannelClient extends Disposable implements ILoggerService {
	declare readonly _serviceBrand: undefined;

	private readonly _onDidChangeLogLevel = this._register(new Emitter<LogLevel>());
	readonly onDidChangeLogLevel: Event<LogLevel> = this._onDidChangeLogLevel.event;

	private readonly _onDidChangeVisibility = this._register(new Emitter<[URI, boolean]>());
	readonly onDidChangeVisibility: Event<[URI, boolean]> = this._onDidChangeVisibility.event;

	private readonly _onDidChangeLoggers = this._register(new Emitter<{ added: ILoggerResource[]; removed: ILoggerResource[] }>());
	readonly onDidChangeLoggers: Event<{ added: ILoggerResource[]; removed: ILoggerResource[] }> = this._onDidChangeLoggers.event;

	private _loggers: Map<string, TauriLogger> = new Map();
	private _globalLogLevel: LogLevel = LogLevel.Info;

	constructor(
		logLevel: LogLevel,
		private _logsHome: URI,
		loggers: ILoggerResource[]
	) {
		super();

		this._globalLogLevel = logLevel;

		// 注册已有的 logger
		for (const loggerResource of loggers) {
			this.registerLogger(loggerResource);
		}
	}

	get logsHome(): URI {
		return this._logsHome;
	}

	get logLevel(): LogLevel {
		return this._globalLogLevel;
	}

	getRegisteredLoggers(): Iterable<ILoggerResource> {
		return this._loggers.values();
	}

	createConsoleMainLogger(): ILogger {
		return new ConsoleLogger();
	}

	createFileLogger(resource: URI, options?: ILoggerOptions): ILogger {
		const logger = new TauriLogger(resource, this._globalLogLevel, options);
		this._loggers.set(resource.toString(), logger);
		return logger;
	}

	registerLogger(logger: ILoggerResource): void {
		const resource = logger.resource;
		const loggerInstance = new TauriLogger(resource, this._globalLogLevel, logger.options);
		this._loggers.set(resource.toString(), loggerInstance);
	}

	deregisterLogger(resource: URI): void;
	deregisterLogger(id: string): void;
	deregisterLogger(arg: URI | string): void {
		const key = arg instanceof URI ? arg.toString() : arg;
		const logger = this._loggers.get(key);
		if (logger) {
			logger.dispose();
			this._loggers.delete(key);
		}
	}

	setLogLevel(level: LogLevel): void;
	setLogLevel(resource: URI, level: LogLevel): void;
	setLogLevel(arg1: any, arg2?: any): void {
		if (arg2 !== undefined) {
			// 设置特定 logger 的级别
			const resource = arg1 as URI;
			const level = arg2 as LogLevel;
			const logger = this._loggers.get(resource.toString());
			if (logger) {
				logger.setLevel(level);
			}
		} else {
			// 设置全局级别
			this._globalLogLevel = arg1 as LogLevel;
			this._onDidChangeLogLevel.fire(this._globalLogLevel);

			// 更新所有 logger 的级别
			for (const logger of this._loggers.values()) {
				logger.setLevel(this._globalLogLevel);
			}
		}
	}

	setVisibility(resource: URI, visibility: boolean): void {
		this._onDidChangeVisibility.fire([resource, visibility]);
	}

	flush(): void {
		for (const logger of this._loggers.values()) {
			logger.flush();
		}
	}
}

/**
 * Tauri Logger 实现
 * 通过 invoke 调用 Rust 侧的日志命令
 */
class TauriLogger extends Disposable implements ILogger {
	private readonly _onDidChangeLogLevel = this._register(new Emitter<LogLevel>());
	readonly onDidChangeLogLevel: Event<LogLevel> = this._onDidChangeLogLevel.event;

	private _level: LogLevel;
	private _loggerId: string | null = null;

	constructor(
		private readonly _resource: URI,
		initialLevel: LogLevel,
		private readonly _options?: ILoggerOptions
	) {
		super();
		this._level = initialLevel;
		this._initializeLogger();
	}

	private async _initializeLogger(): Promise<void> {
		try {
			const level = this._logLevelToString(this._level);
			const rotating = this._options?.rotating ?? true;
			const donot_use_formatters = this._options?.donotUseFormatters ?? false;

			// 调用 Rust 创建 logger
			this._loggerId = await invoke<string>('log_create_logger', {
				name: this._resource.path.split('/').pop() || 'default',
				filepath: this._resource.fsPath,
				rotating,
				donot_use_formatters,
				level
			});
		} catch (error) {
			console.warn('[TauriLogger] Failed to initialize logger:', error);
		}
	}

	getLevel(): LogLevel {
		return this._level;
	}

	setLevel(level: LogLevel): void {
		this._level = level;
		this._onDidChangeLogLevel.fire(level);

		if (this._loggerId) {
			invoke('log_set_level', {
				loggerId: this._loggerId,
				level: this._logLevelToString(level)
			}).catch(err => console.warn('[TauriLogger] Failed to set level:', err));
		}
	}

	trace(message: string, ...args: unknown[]): void {
		this._log(LogLevel.Trace, message, args);
	}

	debug(message: string, ...args: unknown[]): void {
		this._log(LogLevel.Debug, message, args);
	}

	info(message: string, ...args: unknown[]): void {
		this._log(LogLevel.Info, message, args);
	}

	warn(message: string, ...args: unknown[]): void {
		this._log(LogLevel.Warning, message, args);
	}

	error(message: string | Error, ...args: unknown[]): void {
		this._log(LogLevel.Error, typeof message === 'string' ? message : message.message, args);
	}

	private _log(level: LogLevel, message: string, args: unknown[]): void {
		if (level < this._level) {
			return;
		}

		if (this._loggerId) {
			const formattedMessage = args.length > 0
				? `${message} ${args.map(a => JSON.stringify(a)).join(' ')}`
				: message;

			invoke('log_write', {
				loggerId: this._loggerId,
				level: this._logLevelToString(level),
				message: formattedMessage
			}).catch(err => console.warn('[TauriLogger] Failed to write log:', err));
		}
	}

	flush(): void {
		if (this._loggerId) {
			invoke('log_flush', { loggerId: this._loggerId }).catch(err =>
				console.warn('[TauriLogger] Failed to flush:', err)
			);
		}
	}

	override dispose(): void {
		super.dispose();

		if (this._loggerId) {
			invoke('log_drop', { loggerId: this._loggerId }).catch(err =>
				console.warn('[TauriLogger] Failed to drop logger:', err)
			);
		}
	}

	private _logLevelToString(level: LogLevel): string {
		switch (level) {
			case LogLevel.Trace: return 'trace';
			case LogLevel.Debug: return 'debug';
			case LogLevel.Info: return 'info';
			case LogLevel.Warning: return 'warn';
			case LogLevel.Error: return 'error';
			case LogLevel.Off: return 'off';
			default: return 'info';
		}
	}
}

/**
 * 控制台 Logger（用于主进程日志）
 */
class ConsoleLogger implements ILogger {
	readonly onDidChangeLogLevel: Event<LogLevel> = Event.None;

	getLevel(): LogLevel {
		return LogLevel.Info;
	}

	setLevel(_level: LogLevel): void {
		// 控制台 logger 不支持级别设置
	}

	trace(message: string, ...args: unknown[]): void {
		console.trace(message, ...args);
	}

	debug(message: string, ...args: unknown[]): void {
		console.debug(message, ...args);
	}

	info(message: string, ...args: unknown[]): void {
		console.info(message, ...args);
	}

	warn(message: string, ...args: unknown[]): void {
		console.warn(message, ...args);
	}

	error(message: string | Error, ...args: unknown[]): void {
		console.error(message, ...args);
	}

	flush(): void {
		// 控制台不需要 flush
	}

	dispose(): void {
		// 无需清理
	}
}
