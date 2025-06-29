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
				this.reconnectTimeout = setTimeout(() => {
					this.connect(this.currentUrl!);
				}, 1000);
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