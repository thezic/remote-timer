import { formatTimeFromSeconds, seconds } from './time';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'closed';
type Message = {
	current_time: number;
	is_running: boolean;
	client_count: number;
	target_time: number;
};
type Command = { type: 'StartTimer' } | { type: 'StopTimer' } | { type: 'SetTime'; time: number };

class AbortError extends Error {
	constructor() {
		super('abort');
		Object.setPrototypeOf(this, new.target.prototype);
	}
}

class SocketManager {
	private socket: WebSocket | undefined;
	private connectionId: number = 0;
	private listeners: (() => void)[] = [];

	create(url: string): Promise<WebSocket> {
		this.dispose();
		this.connectionId++;
		
		return new Promise((resolve, reject) => {
			const socket = new WebSocket(url);
			this.socket = socket;
			
			const onOpen = () => {
				cleanup();
				resolve(socket);
			};
			
			const onError = () => {
				cleanup();
				reject(new Error('WebSocket connection failed'));
			};
			
			const cleanup = () => {
				socket.removeEventListener('open', onOpen);
				socket.removeEventListener('error', onError);
			};
			
			socket.addEventListener('open', onOpen, { once: true });
			socket.addEventListener('error', onError, { once: true });
		});
	}

	addListener(listener: () => void) {
		this.listeners.push(listener);
	}

	getCurrent(): WebSocket | undefined {
		return this.socket;
	}

	getConnectionId(): number {
		return this.connectionId;
	}

	dispose() {
		if (this.socket) {
			this.socket.close();
			this.socket = undefined;
		}
		
		this.listeners.forEach(cleanup => cleanup());
		this.listeners = [];
	}
}

class ReconnectionStrategy {
	private attempts = 0;
	private maxAttempts = 10;
	private baseDelay = 1000;
	private maxDelay = 30000;

	shouldReconnect(): boolean {
		return this.attempts < this.maxAttempts;
	}

	getDelay(): number {
		const delay = Math.min(
			this.baseDelay * Math.pow(2, this.attempts) + Math.random() * 1000,
			this.maxDelay
		);
		this.attempts++;
		return delay;
	}

	reset() {
		this.attempts = 0;
	}
}

export class TimerService {
	state: ConnectionState = $state('disconnected');
	time = $state(0);
	targetTime = $state(0);
	isRunning = $state(false);
	numberOfClients = $state(0);

	timeSeconds = $derived(seconds(this.time));
	targetSeconds = $derived(seconds(this.targetTime));
	remainingSeconds = $derived(this.targetSeconds - this.timeSeconds);

	private socketManager = new SocketManager();
	private reconnectionStrategy = new ReconnectionStrategy();
	private connectionPromise: Promise<void> | undefined;
	private reconnectionTimeout: number | undefined;
	private currentUrl: string | undefined;
	private visibilityChangeHandler: (() => void) | undefined;
	private connectionValidationTimeout: number | undefined;

	constructor() {
		this.setupPageVisibilityHandling();
	}

	async connect(url: string): Promise<void> {
		if (WebSocket == undefined) {
			return;
		}

		// State guard: only allow connection from disconnected state
		if (this.state !== 'disconnected') {
			console.warn(`Cannot connect from state: ${this.state}`);
			return;
		}

		// Return existing connection attempt if in progress
		if (this.connectionPromise) {
			return this.connectionPromise;
		}

		this.currentUrl = url;
		this.connectionPromise = this.establishConnection(url, false);
		
		try {
			await this.connectionPromise;
		} finally {
			this.connectionPromise = undefined;
		}
	}

	close() {
		this.state = 'closed';
		this.socketManager.dispose();
		this.clearReconnectionTimeout();
		this.clearConnectionValidation();
		this.cleanupPageVisibilityHandling();
		this.connectionPromise = undefined;
		this.currentUrl = undefined;
		this.numberOfClients = 0;
	}

	private clearReconnectionTimeout() {
		if (this.reconnectionTimeout) {
			clearTimeout(this.reconnectionTimeout);
			this.reconnectionTimeout = undefined;
		}
	}

	private clearConnectionValidation() {
		if (this.connectionValidationTimeout) {
			clearTimeout(this.connectionValidationTimeout);
			this.connectionValidationTimeout = undefined;
		}
	}

	private setupPageVisibilityHandling() {
		if (typeof document === 'undefined') {
			return; // Server-side rendering
		}

		this.visibilityChangeHandler = () => {
			if (document.visibilityState === 'visible') {
				this.handlePageVisible();
			}
		};

		document.addEventListener('visibilitychange', this.visibilityChangeHandler);
	}

	private cleanupPageVisibilityHandling() {
		if (this.visibilityChangeHandler && typeof document !== 'undefined') {
			document.removeEventListener('visibilitychange', this.visibilityChangeHandler);
			this.visibilityChangeHandler = undefined;
		}
	}

	private handlePageVisible() {
		// When page becomes visible, validate connection health
		// This is especially important for mobile Safari
		if (this.state === 'connected' && this.currentUrl) {
			this.validateConnection();
		} else if (this.state === 'disconnected' && this.currentUrl) {
			// Auto-reconnect if we were disconnected while in background
			this.connect(this.currentUrl);
		}
	}

	private validateConnection() {
		const socket = this.socketManager.getCurrent();
		
		if (!socket || socket.readyState !== WebSocket.OPEN) {
			// Connection is not actually open, force reconnection
			if (this.currentUrl && this.state === 'connected') {
				this.forceReconnection();
			}
			return;
		}

		// Send a ping to verify connection is actually working
		// If no response within reasonable time, assume connection is stale
		const originalClientCount = this.numberOfClients;
		
		this.connectionValidationTimeout = setTimeout(() => {
			// If client count hasn't been updated, connection might be stale
			if (this.numberOfClients === originalClientCount && this.currentUrl) {
				this.forceReconnection();
			}
		}, 3000); // 3 second timeout for validation
	}

	private forceReconnection() {
		if (this.state === 'closed' || !this.currentUrl) {
			return;
		}

		// Clean up current connection and force new one
		this.socketManager.dispose();
		this.clearReconnectionTimeout();
		this.clearConnectionValidation();
		
		if (!this.connectionPromise) {
			this.connectionPromise = this.establishConnection(this.currentUrl, true);
			this.connectionPromise.finally(() => {
				this.connectionPromise = undefined;
			});
		}
	}

	private async establishConnection(url: string, isReconnection: boolean): Promise<void> {
		try {
			this.state = isReconnection ? 'reconnecting' : 'connecting';
			this.numberOfClients = 0;

			const socket = await this.socketManager.create(url);
			const connectionId = this.socketManager.getConnectionId();

			// Wire up event handlers for current connection
			this.wireSocketEvents(socket, connectionId);

			// Connection successful
			this.state = 'connected';
			this.reconnectionStrategy.reset();
			
		} catch (error) {
			if (this.state === 'closed') {
				return; // Service was closed during connection attempt
			}

			if (isReconnection && this.reconnectionStrategy.shouldReconnect()) {
				// Schedule next reconnection attempt
				const delay = this.reconnectionStrategy.getDelay();
				this.reconnectionTimeout = setTimeout(() => {
					if (this.state !== 'closed' && this.currentUrl) {
						this.connectionPromise = this.establishConnection(this.currentUrl, true);
						this.connectionPromise.finally(() => {
							this.connectionPromise = undefined;
						});
					}
				}, delay);
			} else {
				// Give up reconnecting or initial connection failed
				this.state = 'disconnected';
			}
			
			if (!isReconnection) {
				throw error; // Propagate initial connection errors
			}
		}
	}

	private wireSocketEvents(socket: WebSocket, connectionId: number) {
		const handleConnectionLoss = () => {
			// Only handle if this is still the current connection
			if (connectionId !== this.socketManager.getConnectionId()) {
				return;
			}

			if (this.state === 'closed') {
				return;
			}

			// Start reconnection if we have a URL and not already reconnecting
			if (this.currentUrl && this.state === 'connected' && !this.connectionPromise) {
				this.connectionPromise = this.establishConnection(this.currentUrl, true);
				this.connectionPromise.finally(() => {
					this.connectionPromise = undefined;
				});
			}
		};

		const handleMessage = (event: MessageEvent) => {
			// Only handle messages from current connection
			if (connectionId !== this.socketManager.getConnectionId()) {
				return;
			}

			// Clear connection validation timeout since we received a message
			this.clearConnectionValidation();

			try {
				const msg = JSON.parse(event.data) as Message;
				this.time = msg.current_time;
				this.targetTime = msg.target_time;
				this.isRunning = msg.is_running;
				this.numberOfClients = msg.client_count;
			} catch (error) {
				console.error('Failed to parse WebSocket message:', error);
			}
		};

		// Add event listeners and register cleanup
		socket.addEventListener('close', handleConnectionLoss);
		socket.addEventListener('error', handleConnectionLoss);
		socket.addEventListener('message', handleMessage);

		// Register cleanup function
		this.socketManager.addListener(() => {
			socket.removeEventListener('close', handleConnectionLoss);
			socket.removeEventListener('error', handleConnectionLoss);
			socket.removeEventListener('message', handleMessage);
		});
	}

	private sendMessage(command: Command) {
		const socket = this.socketManager.getCurrent();
		if (socket && socket.readyState === WebSocket.OPEN) {
			socket.send(JSON.stringify(command));
		}
	}

	startTimer() {
		this.sendMessage({ type: 'StartTimer' });
	}

	stopTimer() {
		this.sendMessage({ type: 'StopTimer' });
	}

	setTime(seconds: number) {
		this.sendMessage({ type: 'SetTime', time: seconds * 1000 });
	}
}
