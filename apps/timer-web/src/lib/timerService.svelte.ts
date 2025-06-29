import { seconds } from './time';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'closed';
type Message = {
	current_time: number;
	is_running: boolean;
	client_count: number;
	target_time: number;
};
type Command = { type: 'StartTimer' } | { type: 'StopTimer' } | { type: 'SetTime'; time: number };

export class TimerService {
	state: ConnectionState = $state('disconnected');
	time = $state(0);
	targetTime = $state(0);
	isRunning = $state(false);
	numberOfClients = $state(0);

	timeSeconds = $derived(seconds(this.time));
	targetSeconds = $derived(seconds(this.targetTime));
	remainingSeconds = $derived(this.targetSeconds - this.timeSeconds);

	private socket: WebSocket | undefined;
	private reconnectTimeout: number | undefined;
	private isConnecting = false;
	private currentUrl: string | undefined;
	private retryCount = 0;
	private maxRetries = 8; // Max ~4 minutes of retries
	private lastMessageTime = 0;
	private visibilityHandler: (() => void) | undefined;
	private onlineHandler: (() => void) | undefined;

	constructor() {
		this.setupNetworkMonitoring();
		this.setupVisibilityHandling();
	}

	async connect(url: string): Promise<void> {
		if (typeof WebSocket === 'undefined') {
			return;
		}

		// Prevent multiple concurrent connections
		if (this.isConnecting) {
			return;
		}

		this.isConnecting = true;
		this.currentUrl = url;
		
		try {
			this.cleanup();
			this.state = 'connecting';
			this.numberOfClients = 0;

			this.socket = await this.createSocket(url);
			this.wireSocketEvents(url);
			this.state = 'connected';
			this.retryCount = 0; // Reset on successful connection
		} catch (error) {
			this.state = 'disconnected';
			throw error;
		} finally {
			this.isConnecting = false;
		}
	}

	close() {
		this.state = 'closed';
		this.cleanup();
		this.cleanupEventListeners();
		this.currentUrl = undefined;
		this.numberOfClients = 0;
	}

	private cleanup() {
		if (this.socket) {
			this.socket.close();
			this.socket = undefined;
		}
		
		if (this.reconnectTimeout) {
			clearTimeout(this.reconnectTimeout);
			this.reconnectTimeout = undefined;
		}
	}

	private cleanupEventListeners() {
		if (this.visibilityHandler && typeof document !== 'undefined') {
			document.removeEventListener('visibilitychange', this.visibilityHandler);
			this.visibilityHandler = undefined;
		}
		
		if (this.onlineHandler && typeof window !== 'undefined') {
			window.removeEventListener('online', this.onlineHandler);
			this.onlineHandler = undefined;
		}
	}

	private setupNetworkMonitoring() {
		if (typeof window !== 'undefined' && 'navigator' in window && 'onLine' in navigator) {
			this.onlineHandler = () => this.handleNetworkOnline();
			window.addEventListener('online', this.onlineHandler);
		}
	}

	private setupVisibilityHandling() {
		if (typeof document !== 'undefined') {
			this.visibilityHandler = () => {
				if (document.visibilityState === 'visible' && this.state === 'connected') {
					this.validateConnection();
				}
			};
			document.addEventListener('visibilitychange', this.visibilityHandler);
		}
	}

	private handleNetworkOnline() {
		if (this.state === 'disconnected' && this.currentUrl) {
			// Reset retry count when network comes back online
			this.retryCount = 0;
			this.connect(this.currentUrl);
		}
	}

	private validateConnection() {
		// If no messages received in 10 seconds, connection likely stale
		if (Date.now() - this.lastMessageTime > 10000 && this.currentUrl) {
			this.cleanup();
			this.connect(this.currentUrl);
		}
	}

	private getRetryDelay(): number {
		return Math.min(1000 * Math.pow(2, this.retryCount), 30000);
	}

	private createSocket(url: string): Promise<WebSocket> {
		return new Promise((resolve, reject) => {
			const socket = new WebSocket(url);

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

	private wireSocketEvents(url: string) {
		if (!this.socket) return;

		const socket = this.socket;

		const handleReconnect = () => {
			if (this.state === 'closed' || !this.currentUrl) {
				return;
			}

			// Only reconnect if this is still the current socket
			if (socket === this.socket) {
				// Check retry limits and network status
				if (this.retryCount >= this.maxRetries) {
					console.warn('Max reconnection attempts reached');
					this.state = 'disconnected';
					return;
				}

				// Don't retry if offline
				if (typeof navigator !== 'undefined' && 'onLine' in navigator && !navigator.onLine) {
					this.state = 'disconnected';
					return;
				}

				this.retryCount++;
				const delay = this.getRetryDelay();
				
				this.reconnectTimeout = setTimeout(() => {
					this.connect(this.currentUrl!);
				}, delay);
			}
		};

		const handleMessage = (event: MessageEvent) => {
			// Only handle messages if this is still the current socket
			if (socket !== this.socket) {
				return;
			}

			try {
				const msg = JSON.parse(event.data) as Message;
				this.time = msg.current_time;
				this.targetTime = msg.target_time;
				this.isRunning = msg.is_running;
				this.numberOfClients = msg.client_count;
				this.lastMessageTime = Date.now();
			} catch (error) {
				console.error('Failed to parse WebSocket message:', error);
			}
		};

		socket.addEventListener('close', handleReconnect, { once: true });
		socket.addEventListener('error', handleReconnect, { once: true });
		socket.addEventListener('message', handleMessage);
	}

	private sendMessage(command: Command) {
		if (this.socket && this.socket.readyState === WebSocket.OPEN) {
			this.socket.send(JSON.stringify(command));
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